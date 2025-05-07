use std::fs::OpenOptions;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Drop a CSV file's header.

Usage:
    xan behead [options] [<input>]
    xan guillotine [options] [<input>]

behead options:
    -A, --append  Only drop headers if output already exists and
                  is not empty. Requires -o/--output to be set.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_append: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_append && args.flag_output.is_none() {
        Err("-A/--append needs to know where the output will be written!\nPlease provide -o/--output.")?;
    }

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(false);

    let mut rdr = conf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let wtr_conf = Config::new(&args.flag_output);
    let mut wtr = wtr_conf.writer_with_options(
        OpenOptions::new()
            .write(true)
            .create(true)
            .append(args.flag_append),
    )?;
    let mut record = csv::ByteRecord::new();

    if args.flag_append && wtr_conf.path.unwrap().metadata()?.len() == 0 {
        wtr.write_byte_record(&headers)?;
    }

    while rdr.read_byte_record(&mut record)? {
        wtr.write_byte_record(&record)?;
    }

    Ok(wtr.flush()?)
}
