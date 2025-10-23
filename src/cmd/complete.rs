use std::io::{stdout, Write};

use csv::StringRecord;

use crate::config::{Config, Delimiter};
use crate::dates;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

Usage:
    xan complete [options] <columns> [<input>]
    xan complete --help

complete options:
    -m, --min <num>          The minimum value to start completing from.
                             Default is the first one.
    -M, --max <num>          The maximum value to complete to.
                             Default is the last one.
    -z, --zero <value>       The value to fill in the completed rows.
                             Default is an empty string.
    --check                  Check that the input is complete.
    -D, --dates              Set to indicate your values are dates (supporting year, year-month or
                             year-month-day).

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
    arg_columns: SelectColumns,
    arg_input: Option<String>,
    flag_min: Option<String>,
    flag_max: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_zero: Option<String>,
    flag_check: bool,
    flag_dates: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns)
        .delimiter(args.flag_delimiter);

    let mut wtr_opt = if args.flag_check {
        None
    } else {
        Some(Config::new(&args.flag_output).writer()?)
    };

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    // TODO: use a ByteRecord
    let mut record = StringRecord::new();

    let sel = rconf.selection(&headers)?;

    if let Some(wtr) = wtr_opt.as_mut() {
        wtr.write_record(&headers)?;
    }

    let zero = args.flag_zero.unwrap_or_else(|| "".to_string());

    if args.flag_dates {
        let mut index: Option<dates::PartialDate> = None;

        if let Some(min) = &args.flag_min {
            if let Some(max) = &args.flag_max {
                if dates::parse_partial_date(&min).unwrap().into_inner()
                    > dates::parse_partial_date(&max).unwrap().into_inner()
                {
                    Err("min cannot be greater than max")?;
                }
            }
            index = Some(dates::parse_partial_date(&min).ok_or("invalid min date")?);
        }

        while rdr.read_record(&mut record)? {
            let value = dates::parse_partial_date(&sel.select(&record).next().unwrap().to_string())
                .ok_or("invalide date in file")?;

            if let Some(min) = &args.flag_min {
                // skip values below min of the range
                // TODO: handle min/max properly for dates
                if value.clone().into_inner() < dates::parse_partial_date(min).unwrap().into_inner()
                {
                    continue;
                }
            }

            if let Some(max) = &args.flag_max {
                // stop completing or checking if we go over max of the range
                if value.clone().into_inner() > dates::parse_partial_date(max).unwrap().into_inner()
                {
                    break;
                }
            }

            if index.clone().is_some() {
                if let Some(wtr) = wtr_opt.as_mut() {
                    while value.clone().into_inner() > index.clone().unwrap().into_inner() {
                        let mut new_record = StringRecord::new();
                        for cell in sel.indexed_mask(record.len()) {
                            if cell.is_some() {
                                new_record.push_field(&dates::format_partial_date(
                                    index.clone().unwrap().as_unit(),
                                    &index.clone().unwrap().into_inner(),
                                ));
                            } else {
                                new_record.push_field(&zero);
                            }
                        }
                        index = Some(
                            dates::parse_partial_date(&dates::format_partial_date(
                                index.clone().unwrap().as_unit(),
                                &dates::next_partial_date(
                                    index.clone().unwrap().as_unit(),
                                    &index.clone().unwrap().into_inner(),
                                ),
                            ))
                            .unwrap(),
                        );
                        wtr.write_record(&new_record)?;
                    }
                } else {
                    // in case of using min or if there are repeated values
                    if value.clone().into_inner() < index.clone().unwrap().into_inner() {
                        continue;
                    }
                    if value != index.clone().unwrap() {
                        Err(format!(
                            "file is not complete: missing value {:?}",
                            index.clone().unwrap()
                        ))?;
                    }
                }
            } else {
                index = Some(value);
            }
            let unit = index.clone().unwrap().as_unit();
            index = Some(
                dates::parse_partial_date(&dates::format_partial_date(
                    unit,
                    &dates::next_partial_date(unit, &index.clone().unwrap().into_inner()),
                ))
                .unwrap(),
            );
            if let Some(wtr) = wtr_opt.as_mut() {
                wtr.write_record(&record)?;
            }
        }

        if let Some(max) = args.flag_max {
            let max = dates::parse_partial_date(&max).ok_or("invalid max date")?;
            if let Some(wtr) = wtr_opt.as_mut() {
                while index.clone().is_some()
                    && index.clone().unwrap().into_inner() <= max.clone().into_inner()
                {
                    let mut new_record = StringRecord::new();
                    for cell in sel.indexed_mask(headers.len()) {
                        if cell.is_some() {
                            new_record.push_field(&dates::format_partial_date(
                                index.clone().unwrap().as_unit(),
                                &index.clone().unwrap().into_inner(),
                            ));
                        } else {
                            new_record.push_field(&zero);
                        }
                    }
                    let unit = index.clone().unwrap().as_unit();
                    index = Some(
                        dates::parse_partial_date(&dates::format_partial_date(
                            unit,
                            &dates::next_partial_date(unit, &index.clone().unwrap().into_inner()),
                        ))
                        .unwrap(),
                    );
                    wtr.write_record(&new_record)?;
                }
            } else {
                if index.clone().unwrap().into_inner() <= max.clone().into_inner() {
                    Err(format!(
                        "file is not complete: missing value {:?}",
                        index.clone().unwrap()
                    ))?;
                }
            }
        }
    } else {
        let mut index: Option<i64> = None;

        if let Some(min) = &args.flag_min {
            if let Some(max) = &args.flag_max {
                if min.parse::<i64>().unwrap() > max.parse::<i64>().unwrap() {
                    Err("min cannot be greater than max")?;
                }
            }
            index = Some(min.parse::<i64>().unwrap());
        }

        while rdr.read_record(&mut record)? {
            let value = sel.select(&record).next().unwrap().parse::<i64>().unwrap();

            if let Some(min) = &args.flag_min {
                // skip values below min of the range
                if value < min.parse::<i64>().unwrap() {
                    continue;
                }
            }

            if let Some(max) = &args.flag_max {
                // stop completing or checking if we go over max of the range
                if value > max.parse::<i64>().unwrap() {
                    break;
                }
            }

            if index.is_some() {
                if let Some(wtr) = wtr_opt.as_mut() {
                    while value > index.unwrap() {
                        let mut new_record = StringRecord::new();
                        for cell in sel.indexed_mask(record.len()) {
                            if cell.is_some() {
                                new_record.push_field(&index.unwrap().to_string());
                            } else {
                                new_record.push_field(&zero);
                            }
                        }
                        index = Some(index.unwrap() + 1);
                        wtr.write_record(&new_record)?;
                    }
                } else {
                    // in case of using min or if there are repeated values
                    if value < index.unwrap() {
                        continue;
                    }
                    if value > index.unwrap() {
                        Err(format!(
                            "file is not complete: missing value {}",
                            index.unwrap()
                        ))?;
                    }
                }
            } else {
                index = Some(value);
            }
            index = Some(index.unwrap() + 1);
            if let Some(wtr) = wtr_opt.as_mut() {
                wtr.write_record(&record)?;
            }
        }

        if let Some(max) = args.flag_max {
            let max = max.parse::<i64>().unwrap();
            if let Some(wtr) = wtr_opt.as_mut() {
                while index.is_some() && index.unwrap() <= max {
                    let mut new_record = StringRecord::new();
                    for cell in sel.indexed_mask(headers.len()) {
                        if cell.is_some() {
                            new_record.push_field(&index.unwrap().to_string());
                        } else {
                            new_record.push_field(&zero);
                        }
                    }
                    index = Some(index.unwrap() + 1);
                    wtr.write_record(&new_record)?;
                }
            } else {
                if index.unwrap() <= max {
                    Err(format!(
                        "file is not complete: missing value {}",
                        index.unwrap()
                    ))?;
                }
            }
        }
    }

    if let Some(wtr) = wtr_opt.as_mut() {
        Ok(wtr.flush()?)
    } else {
        writeln!(&mut stdout(), "file is complete!")?;
        Ok(())
    }
}
