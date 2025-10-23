use std::cmp::Ordering;
use std::fmt;
use std::io::{stdout, Write};
use std::str;

use csv::ByteRecord;

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
        match (self, other) {
            (ValuesType::Integer(a), ValuesType::Integer(b)) => a.partial_cmp(b),
            (ValuesType::Date(a), ValuesType::Date(b)) => {
                a.clone().into_inner().partial_cmp(&b.clone().into_inner())
            }
            _ => None,
        }
    }
}

impl Ord for ValuesType {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
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

    let mut record = ByteRecord::new();

    let sel = rconf.selection(&headers)?;

    if let Some(wtr) = wtr_opt.as_mut() {
        wtr.write_record(&headers)?;
    }

    let zero = args.flag_zero.unwrap_or_default().into_bytes();

    let mut index: Option<ValuesType> = None;

    if let Some(min) = &args.flag_min {
        let min = ValuesType::new_from(min, args.flag_dates);
        if let Some(max) = &args.flag_max {
            let max = ValuesType::new_from(max, args.flag_dates);
            if min > max {
                Err("min cannot be greater than max")?;
            }
        }
        index = Some(min);
    }

    while rdr.read_byte_record(&mut record)? {
        let value = ValuesType::new_from(
            str::from_utf8(sel.select(&record).next().unwrap()).unwrap(),
            args.flag_dates,
        );

        if let Some(min) = &args.flag_min {
            let min = ValuesType::new_from(min, args.flag_dates);
            // skip values below min of the range
            if value < min {
                continue;
            }
        }

        if let Some(max) = &args.flag_max {
            let max = ValuesType::new_from(max, args.flag_dates);
            // stop completing or checking if we go over max of the range
            if value > max {
                break;
            }
        }

        if index.is_some() {
            if let Some(wtr) = wtr_opt.as_mut() {
                while value > index.clone().unwrap() {
                    let mut new_record = ByteRecord::new();
                    for cell in sel.indexed_mask(record.len()) {
                        if cell.is_some() {
                            new_record.push_field(&index.clone().unwrap().as_bytes());
                        } else {
                            new_record.push_field(&zero);
                        }
                    }
                    index = Some(index.unwrap().next());
                    wtr.write_record(&new_record)?;
                }
            } else {
                // in case of using min or if there are repeated values
                if value < index.clone().unwrap() {
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
        index = Some(index.unwrap().next());
        if let Some(wtr) = wtr_opt.as_mut() {
            wtr.write_record(&record)?;
        }
    }

    if let Some(max) = args.flag_max {
        let max = ValuesType::new_from(&max, args.flag_dates);
        if let Some(wtr) = wtr_opt.as_mut() {
            while index.is_some() && index.clone().unwrap() <= max {
                let mut new_record = ByteRecord::new();
                for cell in sel.indexed_mask(headers.len()) {
                    if cell.is_some() {
                        new_record.push_field(&index.clone().unwrap().as_bytes());
                    } else {
                        new_record.push_field(&zero);
                    }
                }
                index = Some(index.unwrap().next());
                wtr.write_record(&new_record)?;
            }
        } else if index.clone().unwrap() <= max {
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
