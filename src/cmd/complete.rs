use std::io::{stdout, Write};

use csv::StringRecord;

use crate::config::{Config, Delimiter};
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
    flag_min: Option<i32>,
    flag_max: Option<i32>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_zero: Option<String>,
    flag_check: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let sel = rconf.selection(&headers)?;
    let mut index: Option<i32> = None;

    if let Some(min) = args.flag_min {
        index = Some(min);
    }

    let mut record = StringRecord::new();

    if args.flag_check {
        while rdr.read_record(&mut record)? {
            let value = sel
                .select(&record)
                .map(|i| i.parse::<i32>().unwrap())
                .next();

            if index.is_some() {
                if value.unwrap() != index.unwrap() {
                    Err(format!(
                        "file is not complete: missing value {}",
                        index.unwrap()
                    ))?;
                }
            } else {
                index = Some(value.unwrap());
            }
            index = Some(index.unwrap() + 1);
        }

        writeln!(&mut stdout(), "file is complete!")?;

        return Ok(());
    }

    let zero = args.flag_zero.unwrap_or_else(|| "".to_string());

    wtr.write_record(&headers)?;

    while rdr.read_record(&mut record)? {
        let value = sel
            .select(&record)
            .map(|i| i.parse::<i32>().unwrap())
            .next();

        if index.is_some() {
            while value.unwrap() > index.unwrap() {
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
            if index.unwrap() == value.unwrap() {
                index = Some(index.unwrap() + 1);
            }
        } else {
            index = Some(value.unwrap() + 1);
        }
        wtr.write_record(&record)?;
    }

    if let Some(max) = args.flag_max {
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
    }

    Ok(wtr.flush()?)
}
