use csv;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliResult;

static USAGE: &str = "
TODO...

Usage:
    xsv rename [options] <columns> [<input>]
    xsv rename --help

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_columns: String,
    arg_selection: SelectColumns,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    dbg!(&args.arg_columns);

    // let rconfig = Config::new(&args.arg_input)
    //     .delimiter(args.flag_delimiter)
    //     .no_headers(args.flag_no_headers)
    //     .select(args.arg_selection);

    // let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    // let headers = rdr.byte_headers()?.clone();
    // let sel = rconfig.selection(&headers)?;

    // if !rconfig.no_headers {
    //     wtr.write_record(sel.iter().map(|&i| &headers[i]))?;
    // }
    // let mut record = csv::ByteRecord::new();
    // while rdr.read_byte_record(&mut record)? {
    //     wtr.write_record(sel.iter().map(|&i| &record[i]))?;
    // }

    Ok(wtr.flush()?)
}
