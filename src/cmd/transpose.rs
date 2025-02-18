use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Transpose the given CSV file.

This file:

A,B
C,D

Will become:

A,C
B,D

Usage:
    xan transpose [options] [<input>]
    xan t [options] [<input>]
    xan transpose --help
    xan t --help

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(true);

    let rdr = rconfig.reader()?;

    let records = rdr.into_byte_records().collect::<Result<Vec<_>, _>>()?;

    if records.is_empty() {
        return Ok(());
    }

    let mut wtr = Config::new(&args.flag_output).writer()?;
    let mut output_record = csv::ByteRecord::new();

    let columns = records[0].len();

    for i in 0..columns {
        output_record.clear();

        for record in records.iter() {
            output_record.push_field(&record[i]);
        }

        wtr.write_byte_record(&output_record)?;
    }

    Ok(wtr.flush()?)
}
