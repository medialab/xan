use csv;
use indicatif::ProgressBar;

use config::{Config, Delimiter};
use util;
use CliResult;

static USAGE: &str = "
TODO

Usage:
    xan progress [options] [<input>]
    xan progress --help

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(true);

    let mut rdr = conf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let mut record = csv::ByteRecord::new();

    let bar = ProgressBar::new(1000);

    while rdr.read_byte_record(&mut record)? {
        wtr.write_byte_record(&record)?;
        bar.inc(1);
    }

    bar.abandon();

    Ok(wtr.flush()?)
}
