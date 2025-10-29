use std::cmp::Ordering;
use std::fmt;
use std::io::{stdout, Write};
use std::ops::ControlFlow;
use std::str;

use csv::ByteRecord;
use encoding::codec::utf_8::from_utf8;

use crate::config::{Config, Delimiter};
use crate::dates;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

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
    -z, --zero <value>       The value to fill in the new rows.
                             Default is an empty string.
    --check                  Check that the input is complete. When used with
                             either --min or --max, only checks completeness
                             within the specified range.
    -D, --dates              Set to indicate your values are dates (supporting
                             year, year-month or year-month-day).
    --sorted                 Indicate that the input is already sorted. When
                             used without --reverse, the input is sorted in
                             ascending order. When used with --reverse, the
                             input is sorted in descending order.
    -R, --reverse                When used with --sorted, indicate that the input is
                             sorted in descending order. When used
                             without --sorted, the output will be sorted in
                             descending order.

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
    arg_column: SelectColumns,
    arg_input: Option<String>,
    flag_min: Option<String>,
    flag_max: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_zero: Option<String>,
    flag_check: bool,
    flag_dates: bool,
    flag_sorted: bool,
    flag_reverse: bool,
}

enum ValuesType {
    Integer(i64),
    Date(dates::PartialDate),
}

impl Clone for ValuesType {
    fn clone(&self) -> Self {
        match self {
            ValuesType::Integer(i) => ValuesType::Integer(*i),
            ValuesType::Date(d) => ValuesType::Date(d.clone()),
        }
    }
}

impl Eq for ValuesType {}

impl PartialEq for ValuesType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValuesType::Integer(a), ValuesType::Integer(b)) => a == b,
            (ValuesType::Date(a), ValuesType::Date(b)) => {
                a.clone().into_inner() == b.clone().into_inner()
            }
            _ => false,
        }
    }
}

impl PartialOrd for ValuesType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ValuesType {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (ValuesType::Integer(a), ValuesType::Integer(b)) => a.cmp(b),
            (ValuesType::Date(a), ValuesType::Date(b)) => {
                a.clone().into_inner().cmp(&b.clone().into_inner())
            }
            _ => unreachable!(),
        }
    }
}

impl fmt::Debug for ValuesType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValuesType::Integer(i) => write!(f, "{}", i),
            ValuesType::Date(d) => write!(f, "{}", d.clone().into_inner()),
        }
    }
}

impl ValuesType {
    fn new_from(s: &str, is_date: bool) -> Self {
        if is_date {
            ValuesType::Date(dates::parse_partial_date(s).unwrap())
        } else {
            ValuesType::Integer(s.parse::<i64>().unwrap())
        }
    }

    fn next(&self) -> ValuesType {
        match self {
            ValuesType::Integer(i) => ValuesType::Integer(i + 1),
            ValuesType::Date(d) => ValuesType::Date(
                dates::parse_partial_date(&dates::format_partial_date(
                    d.as_unit(),
                    &dates::next_partial_date(d.as_unit(), &d.clone().into_inner()),
                ))
                .unwrap(),
            ),
        }
    }

    fn previous(&self) -> ValuesType {
        match self {
            ValuesType::Integer(i) => ValuesType::Integer(i - 1),
            ValuesType::Date(d) => ValuesType::Date(
                dates::parse_partial_date(&dates::format_partial_date(
                    d.as_unit(),
                    &dates::previous_partial_date(d.as_unit(), &d.clone().into_inner()),
                ))
                .unwrap(),
            ),
        }
    }

    fn as_bytes(&self) -> Vec<u8> {
        match self {
            ValuesType::Integer(i) => i.to_string().as_bytes().to_vec(),
            ValuesType::Date(ref d) => {
                dates::format_partial_date(d.as_unit(), &d.clone().into_inner())
                    .as_bytes()
                    .to_vec()
            }
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let min: Option<ValuesType> = match &args.flag_min {
        Some(m) => Some(ValuesType::new_from(m, args.flag_dates)),
        None => None,
    };
    let max: Option<ValuesType> = match &args.flag_max {
        Some(m) => Some(ValuesType::new_from(m, args.flag_dates)),
        None => None,
    };

    if min.is_some() && max.is_some() && min.clone().unwrap() > max.clone().unwrap() {
        Err("min cannot be greater than max")?;
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column)
        .delimiter(args.flag_delimiter);

    let mut wtr_opt = if args.flag_check {
        None
    } else {
        Some(Config::new(&args.flag_output).writer()?)
    };

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let sel = rconf.selection(&headers)?;

    if let Some(wtr) = wtr_opt.as_mut() {
        wtr.write_record(&headers)?;
    }

    let zero = args.flag_zero.unwrap_or_default().into_bytes();

    let mut index: Option<ValuesType> = if args.flag_reverse {
        max.clone()
    } else {
        min.clone()
    };

    let mut process_record =
        |record: &ByteRecord, index: &mut Option<ValuesType>| -> CliResult<ControlFlow<()>> {
            let value = ValuesType::new_from(
                str::from_utf8(sel.select(record).next().unwrap()).unwrap(),
                args.flag_dates,
            );

            if min.is_some() && value < min.clone().unwrap() {
                if args.flag_reverse {
                    // stop completing or checking if we go below min of the range
                    return Ok(ControlFlow::Break(()));
                } else {
                    // skip values below min of the range
                    return Ok(ControlFlow::Continue(()));
                }
            }
            if max.is_some() && value > max.clone().unwrap() {
                if args.flag_reverse {
                    // skip values over max of the range
                    return Ok(ControlFlow::Continue(()));
                } else {
                    // stop completing or checking if we go over max of the range
                    return Ok(ControlFlow::Break(()));
                }
            }

            if index.is_some() {
                if let Some(wtr) = wtr_opt.as_mut() {
                    while (args.flag_reverse && value < index.clone().unwrap())
                        || (!args.flag_reverse && value > index.clone().unwrap())
                    {
                        let mut new_record = ByteRecord::new();
                        for cell in sel.indexed_mask(record.len()) {
                            if cell.is_some() {
                                new_record.push_field(&index.clone().unwrap().as_bytes());
                            } else {
                                new_record.push_field(&zero);
                            }
                        }

                        *index = if args.flag_reverse {
                            Some(index.clone().unwrap().previous())
                        } else {
                            Some(index.clone().unwrap().next())
                        };

                        wtr.write_record(&new_record)?;
                    }
                } else {
                    // in case of using min flag (or max flag when flag_reverse is true)
                    // or if there are repeated values
                    if (args.flag_reverse && value > index.clone().unwrap())
                        || (!args.flag_reverse && value < index.clone().unwrap())
                    {
                        return Ok(ControlFlow::Continue(()));
                    }
                    if value != index.clone().unwrap() {
                        Err(format!(
                            "file is not complete: missing value {:?}",
                            index.clone().unwrap()
                        ))?;
                    }
                }
            } else {
                *index = Some(value);
            }

            *index = if args.flag_reverse {
                Some(index.clone().unwrap().previous())
            } else {
                Some(index.clone().unwrap().next())
            };

            if let Some(wtr) = wtr_opt.as_mut() {
                wtr.write_record(record)?;
            }

            Ok(ControlFlow::Continue(()))
        };

    let mut record = ByteRecord::new();

    // process records
    if args.flag_sorted {
        while rdr.read_byte_record(&mut record)? {
            match process_record(&record, &mut index)? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(()) => break,
            }
        }
    } else {
        let mut values_and_records = rdr
            .byte_records()
            .map(|r| -> CliResult<(ValuesType, ByteRecord)> {
                let record = r?;
                let value_and_record = (
                    ValuesType::new_from(
                        from_utf8(sel.select(&record).next().unwrap()).unwrap(),
                        args.flag_dates,
                    ),
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

        for record in records {
            match process_record(record, &mut index)? {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(()) => break,
            }
        }
    }

    if (args.flag_reverse && min.is_some()) || (!args.flag_reverse && max.is_some()) {
        if let Some(wtr) = wtr_opt.as_mut() {
            while index.is_some()
                && ((args.flag_reverse && index.clone().unwrap() >= min.clone().unwrap())
                    || (!args.flag_reverse && index.clone().unwrap() <= max.clone().unwrap()))
            {
                let mut new_record = ByteRecord::new();
                for cell in sel.indexed_mask(headers.len()) {
                    if cell.is_some() {
                        new_record.push_field(&index.clone().unwrap().as_bytes());
                    } else {
                        new_record.push_field(&zero);
                    }
                }

                index = if args.flag_reverse {
                    Some(index.unwrap().previous())
                } else {
                    Some(index.unwrap().next())
                };

                wtr.write_record(&new_record)?;
            }
        } else if (args.flag_reverse && index.clone().unwrap() >= min.clone().unwrap())
            || (!args.flag_reverse && index.clone().unwrap() <= max.clone().unwrap())
        {
            Err(format!(
                "file is not complete: missing value {:?}",
                index.unwrap()
            ))?;
        }
    }

    if let Some(wtr) = wtr_opt.as_mut() {
        Ok(wtr.flush()?)
    } else {
        writeln!(&mut stdout(), "file is complete!")?;
        Ok(())
    }
}
