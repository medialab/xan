use jiff::Unit;
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

Examples:
  Complete integer values in column named "score" from 1 to 10:
    $ xan complete -m 1 -M 10 score input.csv

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
    fn get_value_from_str(&self, cell: &str) -> CliResult<ValuesType> {
        if self.flag_dates {
            ValuesType::new_date(cell)
        } else {
            ValuesType::new_integer(cell)
        }
    }

    fn get_value_from_bytes(&self, cell: &[u8]) -> CliResult<ValuesType> {
        self.get_value_from_str(str::from_utf8(cell).unwrap())
    }
}

#[derive(Debug, PartialEq)]
enum ValuesUnit {
    Integer,
    Date(Unit),
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
    fn new_date(s: &str) -> CliResult<Self> {
        if !dates::could_be_date(s) {
            Err(format!("Invalid date format: {}", s))?;
        }
        Ok(ValuesType::Date(dates::parse_partial_date(s).map_or_else(
            || Err(format!("Invalid date format: {}", s)),
            Ok,
        )?))
    }

    fn new_integer(s: &str) -> CliResult<Self> {
        let v = ValuesType::Integer(
            s.parse::<i64>()
                .map_err(|_| format!("Invalid integer format: {}", s))?,
        );
        Ok(v)
    }

    fn next(&self) -> ValuesType {
        match self {
            ValuesType::Integer(i) => ValuesType::Integer(i + 1),
            ValuesType::Date(d) => ValuesType::Date(PartialDate::from_date(
                dates::next_partial_date(d.as_unit(), d.as_date()),
                d.as_unit(),
            )),
        }
    }

    fn previous(&self) -> ValuesType {
        match self {
            ValuesType::Integer(i) => ValuesType::Integer(i - 1),
            ValuesType::Date(d) => ValuesType::Date(PartialDate::from_date(
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

    fn as_unit(&self) -> ValuesUnit {
        match self {
            ValuesType::Integer(_) => ValuesUnit::Integer,
            ValuesType::Date(d) => ValuesUnit::Date(d.as_unit()),
        }
    }

    fn has_same_unit_as(&self, other: &ValuesType) -> bool {
        self.as_unit() == other.as_unit()
    }

    fn verify_unit(&self, first_seen_value: &mut Option<ValuesType>) -> CliResult<()> {
        if let Some(v) = first_seen_value {
            if self.as_unit() == v.as_unit() {
                Ok(())
            } else {
                Err(format!(
                    "Inconsistent value units: first seen was {:?} and then found {:?}",
                    v.as_unit(),
                    self.as_unit(),
                ))?
            }
        } else {
            *first_seen_value = Some(*self);
            Ok(())
        }
    }
}

fn new_record(
    headers_len: usize,
    column_to_complete_index: usize,
    sel_group_by: &Option<&Selection>,
    groups: &ByteRecord,
    index_value: &[u8],
) -> ByteRecord {
    // let mut new_record = ByteRecord::new();
    let mut new_record = ByteRecord::new();
    let mut group_index = 0;

    for i in 0..headers_len {
        if i == column_to_complete_index {
            new_record.push_field(index_value);
        // i + 1 because Selection::mask(alignement) returns a mask of length alignement
        } else if matches!(sel_group_by, Some(s) if s.mask(i+1)[i]) {
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
        .map(|m| args.get_value_from_str(m.as_str()))
        .transpose()?;
    let max: Option<ValuesType> = args
        .flag_max
        .clone()
        .map(|m| args.get_value_from_str(m.as_str()))
        .transpose()?;

    // to verify that all values have the same unit and type
    let mut first_seen_value: Option<ValuesType> = None;

    if let (Some(min_v), Some(max_v)) = (&min, &max) {
        if !min_v.has_same_unit_as(max_v) {
            Err(format!(
                "min and max have different units: {:?} vs {:?}",
                min_v.as_unit(),
                max_v.as_unit()
            ))?;
        }

        if min_v > max_v {
            Err("min cannot be greater than max")?;
        }
        min_v.verify_unit(&mut first_seen_value)?;
    }

    let mut extent_builder = ExtentBuilder::<ValuesType>::new();

    if let Some(m) = min {
        extent_builder.clamp_min(m);
    }
    if let Some(m) = max {
        extent_builder.clamp_max(m);
    }

    // Will be equal to None if either min or max is not specified.
    // If both min and max are specified, then when processing the input values,
    // there will be no need to found the extreme values of the input range.
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

    let sel_group_by_owned: Option<Selection> = if let Some(ref fgb) = args.flag_groupby {
        Some(fgb.selection(&headers, !args.flag_no_headers)?)
    } else {
        None
    };
    let sel_group_by: Option<&Selection> = sel_group_by_owned.as_ref();

    if matches!(sel_group_by, Some(s) if s.mask(column_to_complete_index + 1)[column_to_complete_index])
    {
        Err("Cannot complete on a column that is also used in --groupby")?;
    }

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
        // Collecting records per group
        if let Some(flag_groupby) = &args.flag_groupby {
            let group_sel = flag_groupby.selection(&headers, !args.flag_no_headers)?;
            while rdr.read_byte_record(&mut record)? {
                let value: ValuesType =
                    args.get_value_from_bytes(&record[column_to_complete_index])?;
                value.verify_unit(&mut first_seen_value)?;
                // Meaning we need to find the extent from the input values
                // (either min or max or both were not specified)
                if extent.is_none() {
                    extent_builder.process(value);
                // Skipping the value if outside of the specified range
                } else if matches!(extent, Some(e) if value < e.min() || value > e.max()) {
                    continue;
                }

                let key = group_sel.select(&record).collect::<ByteRecord>();
                records_per_group.insert_with_or_else(
                    key,
                    || vec![record.clone()],
                    |v| v.push(record.clone()),
                );
            }
        // Collecting all records together (artificial single group)
        } else {
            records_per_group.insert_with(ByteRecord::new(), || {
                rdr.byte_records().collect::<Result<Vec<_>, _>>().unwrap()
            });
        };
    // Input is sorted, no need to collect nor group records (will process them
    // directly later, here just creating an empty group to enter the processing loop)
    } else {
        records_per_group.insert_with(ByteRecord::new(), Vec::new);
    }

    // if extent was not determined yet, do it now
    if extent.is_none() {
        extent = extent_builder.build();
    }

    let min = min.or_else(|| extent.as_ref().map(|e| e.min()));
    let max = max.or_else(|| extent.as_ref().map(|e| e.max()));

    // Can be None if min is None when not using --reverse or max is None when
    //using --reverse (with -S/--sorted), meaning we start completing
    // from the first value in the input
    let current_value: Option<ValuesType> = if args.flag_reverse { max } else { min };

    // closure to process ALREADY SORTED records in a group
    let mut process_records_in_group = |records: &mut dyn Iterator<Item = ByteRecord>,
                                        group_key: &ByteRecord|
     -> CliResult<()> {
        let mut local_current_value: Option<ValuesType> = current_value;

        for record in records {
            let value: ValuesType = args.get_value_from_bytes(&record[column_to_complete_index])?;
            value.verify_unit(&mut first_seen_value)?;

            if matches!(min, Some(m) if value < m) {
                if args.flag_reverse {
                    // stop completing or checking if we go below min of the range
                    break;
                } else {
                    // skip values below min of the range
                    continue;
                }
            }
            if matches!(max, Some(m) if value > m) {
                if args.flag_reverse {
                    // skip values over max of the range
                    continue;
                } else {
                    // stop completing or checking if we go over max of the range
                    break;
                }
            }

            if local_current_value.is_some() {
                // writing missing values
                if let Some(wtr) = wtr_opt.as_mut() {
                    // until we reach the input value, complete missing values
                    while match (args.flag_reverse, local_current_value) {
                        (true, Some(cv)) => cv > value,
                        (false, Some(cv)) => cv < value,
                        _ => false,
                    } {
                        wtr.write_byte_record(&new_record(
                            headers.len(),
                            column_to_complete_index,
                            &sel_group_by,
                            group_key,
                            &local_current_value.unwrap().as_bytes(),
                        ))?;

                        local_current_value =
                            local_current_value.map(|v| v.advance(args.flag_reverse));
                    }
                // checking for completeness
                } else if value != local_current_value.unwrap() {
                    // in case of using min flag (or max flag when flag_reverse is true)
                    // and having 'value' outside of that range,
                    // or if there are repeated values in the input
                    if match (args.flag_reverse, local_current_value) {
                        // If using max flag, this condition being true means
                        // the current value is out of range, ignoring it.
                        // else if conditon is true, means there are repeated values in the input
                        (true, Some(cv)) => cv < value,
                        // if using min flag, this condition being true means
                        // the current value is out of range, ignoring it
                        // else if conditon is true, means there are repeated values in the input
                        (false, Some(cv)) => cv > value,
                        _ => false,
                    } {
                        continue;
                    }
                    Err(format!(
                        "file is not complete: missing value {:?}",
                        local_current_value.unwrap()
                    ))?;
                }
            // meaning we are at the first record of the group
            } else {
                local_current_value = Some(value);
            }

            local_current_value = local_current_value.map(|v| v.advance(args.flag_reverse));

            if let Some(wtr) = wtr_opt.as_mut() {
                wtr.write_byte_record(&record)?;
            }
        }

        // No more input records in the group, but we may need to complete/check
        // to min or max (if set by the flag) depending on the direction
        if (args.flag_reverse && min.is_some()) || (!args.flag_reverse && max.is_some()) {
            // completing/writing missing values
            if let Some(wtr) = wtr_opt.as_mut() {
                // while being within the specified range, complete values
                while local_current_value.is_some()
                    && match (args.flag_reverse, local_current_value) {
                        (true, Some(cv)) if cv >= min.unwrap() => true,
                        (false, Some(cv)) if cv <= max.unwrap() => true,
                        _ => false,
                    }
                {
                    wtr.write_byte_record(&new_record(
                        headers.len(),
                        column_to_complete_index,
                        &sel_group_by,
                        group_key,
                        &local_current_value.unwrap().as_bytes(),
                    ))?;

                    local_current_value = local_current_value.map(|v| v.advance(args.flag_reverse));
                }
            // checking for completeness
            } else if match (args.flag_reverse, local_current_value) {
                // if after processing all input records in the group and
                // 'advancing' to the next value, we are still within the
                // specified range, then the input is not complete
                (true, Some(cv)) if cv >= min.unwrap() => true,
                (false, Some(cv)) if cv <= max.unwrap() => true,
                _ => false,
            } {
                Err(format!(
                    "file is not complete: missing value {:?}",
                    local_current_value.unwrap()
                ))?;
            }
        }

        Ok(())
    };

    // process all records
    for (group_key, records) in records_per_group.iter() {
        if args.flag_sorted && args.flag_groupby.is_none() {
            // NOTE: group_key is empty here, and will be ignored in the processing
            // QUESTION: Am I not allowing memory for every record here (in case input from stdin)?
            process_records_in_group(&mut rdr.byte_records().map(|r| r.unwrap()), group_key)?;
        } else {
            // sorting records in the group
            let mut values_and_records = records
                .iter()
                .map(|record| -> CliResult<(ValuesType, ByteRecord)> {
                    let value_and_record = (
                        args.get_value_from_bytes(&record[column_to_complete_index])?,
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
