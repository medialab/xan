use std::cmp::Ordering;
use std::fmt;
use std::io::{stdout, Write};
use std::str;

use simd_csv::ByteRecord;

use crate::collections::ClusteredInsertHashmap;
use crate::config::{Config, Delimiter};
use crate::dates::{self, PartialDate};
use crate::scales::{Extent, ExtentBuilder};
use crate::select::SelectedColumns;
use crate::select::Selection;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Complete or check on missing values in a column. Can handle integer or date values.
A --min and/or --max flag can be used to specify a range to complete or check.
Note that when completing, if the input contains values outside the specified
range, those values will be removed from the output.
You can specify that the input is already sorted in ascending order on the column
to complete with the --sorted flag, and in descending order using both --sorted
and --reverse, which will make the command faster.
Will by default output in ascending order on the completed column, but you can
use the --reverse flag to output in descending order.
You can also complete values within groups defined by other columns using the --groupby
flag, completing with the same range for each group.
When completing, new rows will be filled with the value specified by the --zero
flag (or an empty string by default) in all columns except the completed column
and the group-by columns (if any).

Examples:
  Complete integer values in column named "score" from 1 to 10, filling new rows with 0:
    $ xan complete -m 1 -M 10 -z 0 score input.csv

  Complete already sorted date values in column named "date":
    $ xan complete -D --sorted date input.csv

  Check that the values (already sorted in descending order) in column named "score" are complete:
    $ xan complete --check --sorted --reverse score input.csv

  Complete integer values in column named "score" within groups defined by columns "name" and "category":
    $ xan complete --groupby name,category score input.csv

Usage:
    xan complete [options] <column> [<input>]
    xan complete --help

complete options:
    -m, --min <value>        The minimum value to start completing from.
                             Default is the first one. Note that if <value> is
                             greater than the minimum value in the input, the
                             rows with values lower than <value> will be removed
                             from the output.
    -M, --max <value>        The maximum value to complete to.
                             Default is the last one. Note that if <value> is
                             lower than the maximum value in the input, the rows
                             with values greater than <value> will be removed
                             from the output.
    --check                  Check that the input is complete. When used with
                             either --min or --max, only checks completeness
                             within the specified range.
    -D, --dates              Set to indicate your values are dates (supporting
                             year, year-month or year-month-day).
    --sorted                 Indicate that the input is already sorted. When
                             used without --reverse, the input is sorted in
                             ascending order. When used with --reverse, the
                             input is sorted in descending order.
    -R, --reverse            When used with --sorted, indicate that the input is
                             sorted in descending order. When used
                             without --sorted, the output will be sorted in
                             descending order.
    -g, --groupby <cols>     Select columns to group by. The completion will be
                             done independently within each group.


Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.

"#;

#[derive(Deserialize, Debug)]
struct Args {
    arg_column: SelectedColumns,
    arg_input: Option<String>,
    flag_min: Option<String>,
    flag_max: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_check: bool,
    flag_dates: bool,
    flag_sorted: bool,
    flag_reverse: bool,
    flag_groupby: Option<SelectedColumns>,
}

impl Args {
    fn get_value_from_str(&self, cell: &str) -> ValuesType {
        if self.flag_dates {
            ValuesType::new_date(cell)
        } else {
            ValuesType::new_integer(cell)
        }
    }

    fn get_value_from_bytes(&self, cell: &[u8]) -> ValuesType {
        self.get_value_from_str(str::from_utf8(cell).unwrap())
    }
}

#[derive(Copy, Clone, PartialEq)]
enum ValuesType {
    Integer(i64),
    Date(dates::PartialDate),
}

impl Eq for ValuesType {}

impl PartialOrd for ValuesType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ValuesType {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (ValuesType::Integer(a), ValuesType::Integer(b)) => a.cmp(b),
            (ValuesType::Date(a), ValuesType::Date(b)) => a.cmp(b),
            _ => unreachable!(),
        }
    }
}

impl fmt::Debug for ValuesType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValuesType::Integer(i) => write!(f, "{}", i),
            ValuesType::Date(d) => write!(f, "{}", d.as_date()),
        }
    }
}

impl ValuesType {
    fn new_date(s: &str) -> Self {
        ValuesType::Date(dates::parse_partial_date(s).unwrap())
    }

    fn new_integer(s: &str) -> Self {
        ValuesType::Integer(s.parse::<i64>().unwrap())
    }

    fn next(&self) -> ValuesType {
        match self {
            ValuesType::Integer(i) => ValuesType::Integer(i + 1),
            ValuesType::Date(d) => ValuesType::Date(PartialDate::new_from_date(
                dates::next_partial_date(d.as_unit(), d.as_date()),
                d.as_unit(),
            )),
        }
    }

    fn previous(&self) -> ValuesType {
        match self {
            ValuesType::Integer(i) => ValuesType::Integer(i - 1),
            ValuesType::Date(d) => ValuesType::Date(PartialDate::new_from_date(
                dates::previous_partial_date(d.as_unit(), d.as_date()),
                d.as_unit(),
            )),
        }
    }

    fn advance(&self, reverse: bool) -> ValuesType {
        if reverse {
            self.previous()
        } else {
            self.next()
        }
    }

    fn as_bytes(&self) -> Vec<u8> {
        match self {
            ValuesType::Integer(i) => i.to_string().into_bytes(),
            ValuesType::Date(ref d) => {
                dates::format_partial_date(d.as_unit(), d.as_date()).into_bytes()
            }
        }
    }
}

fn new_record_with_zeroed_column(
    headers_len: usize,
    column_to_complete_index: usize,
    sel_group_by: &Option<&Selection>,
    groups: &ByteRecord,
    index_value: &[u8],
) -> ByteRecord {
    let mut new_record = ByteRecord::new();
    let mut group_index = 0;

    for i in 0..headers_len {
        if i == column_to_complete_index {
            new_record.push_field(index_value);
        } else if sel_group_by.is_some()
            && sel_group_by
                .as_ref()
                .map(|s| s.contains(i))
                .unwrap_or(false)
        {
            new_record.push_field(&groups[group_index]);
            group_index += 1;
        } else {
            new_record.push_field(b"");
        }
    }
    new_record
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_groupby.is_some() && args.flag_sorted {
        Err("--groupby cannot be used with --sorted")?;
    }

    let min: Option<ValuesType> = args
        .flag_min
        .clone()
        .map(|m| args.get_value_from_str(m.as_str()));
    let max: Option<ValuesType> = args
        .flag_max
        .clone()
        .map(|m| args.get_value_from_str(m.as_str()));

    if matches!((&min, &max), (Some(min), Some(max))
        if min > max)
    {
        Err("min cannot be greater than max")?;
    }

    let mut extent_builder = ExtentBuilder::<ValuesType>::new();

    if let Some(m) = min {
        extent_builder.clamp_min(m);
    }
    if let Some(m) = max {
        extent_builder.clamp_max(m);
    }

    let mut extent: Option<Extent<ValuesType>> = extent_builder.clone().build();

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column.clone())
        .delimiter(args.flag_delimiter);

    let mut wtr_opt = (!(args.flag_check))
        .then(|| Config::new(&args.flag_output).simd_writer())
        .transpose()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let column_to_complete_index = rconf.single_selection(&headers)?;

    if !rconf.no_headers {
        if let Some(wtr) = wtr_opt.as_mut() {
            wtr.write_byte_record(&headers)?;
        }
    }

    let mut record = ByteRecord::new();

    // reading records and grouping them if needed
    let mut records_per_group: ClusteredInsertHashmap<ByteRecord, Vec<ByteRecord>> =
        ClusteredInsertHashmap::new();
    if !args.flag_sorted {
        if args.flag_groupby.is_some() {
            let group_sel = args
                .flag_groupby
                .as_ref()
                .unwrap()
                .selection(&headers, !args.flag_no_headers)?;
            while rdr.read_byte_record(&mut record)? {
                let key = group_sel.select(&record).collect::<ByteRecord>();
                records_per_group.insert_with_or_else(
                    key,
                    || vec![record.clone()],
                    |v| v.push(record.clone()),
                );

                if extent.is_none() {
                    let value = args.get_value_from_bytes(&record[column_to_complete_index]);
                    extent_builder.process(value);
                }
            }
        } else {
            records_per_group.insert_with(ByteRecord::new(), || {
                rdr.byte_records().collect::<Result<Vec<_>, _>>().unwrap()
            });
        };
    } else {
        records_per_group.insert_with(ByteRecord::new(), Vec::new);
    }

    if extent.is_none() {
        extent = extent_builder.build();
    }

    let min = min.or_else(|| extent.as_ref().map(|e| e.min().clone()));
    let max = max.or_else(|| extent.as_ref().map(|e| e.max().clone()));

    let index: Option<ValuesType> = if args.flag_reverse {
        max.clone()
    } else {
        min.clone()
    };

    let sel_group_by_owned: Option<Selection> = if let Some(ref fgb) = args.flag_groupby {
        Some(fgb.selection(&headers, !args.flag_no_headers)?)
    } else {
        None
    };
    let sel_group_by: Option<&Selection> = sel_group_by_owned.as_ref();

    // closure to process ALREADY SORTED records in a group
    let mut process_records_in_group = |records: &mut dyn Iterator<Item = ByteRecord>,
                                        group_key: &ByteRecord|
     -> CliResult<()> {
        let mut local_index: Option<ValuesType> = index.clone();

        for record in records {
            let value = args.get_value_from_bytes(&record[column_to_complete_index]);

            if min.is_some() && value < min.clone().unwrap() {
                if args.flag_reverse {
                    // stop completing or checking if we go below min of the range
                    break;
                } else {
                    // skip values below min of the range
                    continue;
                }
            }
            if max.is_some() && value > max.clone().unwrap() {
                if args.flag_reverse {
                    // skip values over max of the range
                    continue;
                } else {
                    // stop completing or checking if we go over max of the range
                    break;
                }
            }

            if local_index.is_some() {
                if let Some(wtr) = wtr_opt.as_mut() {
                    while (args.flag_reverse && value < local_index.clone().unwrap())
                        || (!args.flag_reverse && value > local_index.clone().unwrap())
                    {
                        wtr.write_byte_record(&new_record_with_zeroed_column(
                            headers.len(),
                            column_to_complete_index,
                            &sel_group_by,
                            group_key,
                            &local_index.clone().unwrap().as_bytes(),
                        ))?;

                        local_index = Some(local_index.clone().unwrap().advance(args.flag_reverse));
                    }
                } else {
                    // in case of using min flag (or max flag when flag_reverse is true)
                    // or if there are repeated values
                    if (args.flag_reverse && value > local_index.clone().unwrap())
                        || (!args.flag_reverse && value < local_index.clone().unwrap())
                    {
                        continue;
                    }
                    if value != local_index.clone().unwrap() {
                        Err(format!(
                            "file is not complete: missing value {:?}",
                            local_index.clone().unwrap()
                        ))?;
                    }
                }
            } else {
                local_index = Some(value);
            }

            local_index = Some(local_index.clone().unwrap().advance(args.flag_reverse));

            if let Some(wtr) = wtr_opt.as_mut() {
                wtr.write_byte_record(&record)?;
            }
        }

        if (args.flag_reverse && min.is_some()) || (!args.flag_reverse && max.is_some()) {
            if let Some(wtr) = wtr_opt.as_mut() {
                while local_index.is_some()
                    && ((args.flag_reverse && local_index.clone().unwrap() >= min.clone().unwrap())
                        || (!args.flag_reverse
                            && local_index.clone().unwrap() <= max.clone().unwrap()))
                {
                    wtr.write_byte_record(&new_record_with_zeroed_column(
                        headers.len(),
                        column_to_complete_index,
                        &sel_group_by,
                        group_key,
                        &local_index.clone().unwrap().as_bytes(),
                    ))?;

                    local_index = Some(local_index.clone().unwrap().advance(args.flag_reverse));
                }
            } else if (args.flag_reverse && local_index.clone().unwrap() >= min.clone().unwrap())
                || (!args.flag_reverse && local_index.clone().unwrap() <= max.clone().unwrap())
            {
                Err(format!(
                    "file is not complete: missing value {:?}",
                    local_index.unwrap()
                ))?;
            }
        }

        Ok(())
    };

    // process all records
    for (group_key, records) in records_per_group.iter() {
        if args.flag_sorted && args.flag_groupby.is_none() {
            // QUESTION: Am I not allowing memory for every record here (in case input from stdin)?
            process_records_in_group(&mut rdr.byte_records().map(|r| r.unwrap()), group_key)?;
        } else {
            // sorting records in the group
            let mut values_and_records = records
                .iter()
                .map(|record| -> CliResult<(ValuesType, ByteRecord)> {
                    let value_and_record = (
                        args.get_value_from_bytes(&record[column_to_complete_index]),
                        record.clone(),
                    );
                    Ok(value_and_record)
                })
                .collect::<Result<Vec<_>, _>>()?;
            values_and_records.sort_by(|a, b| {
                if args.flag_reverse {
                    b.0.cmp(&a.0)
                } else {
                    a.0.cmp(&b.0)
                }
            });
            let records = values_and_records.iter().map(|(_, r)| r);

            process_records_in_group(&mut records.cloned(), group_key)?;
        }
    }

    if let Some(wtr) = wtr_opt.as_mut() {
        Ok(wtr.flush()?)
    } else {
        writeln!(&mut stdout(), "file is complete!")?;
        Ok(())
    }
}
