use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Drop columns of a CSV file using the same DSL as \"xan select\".

Basically a shorthand for the negative selection of \"xan select\".

Usage:
    xan drop [options] [--] <selection> [<input>]
    xan drop --help

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_selection: SelectColumns,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

    args.arg_selection.invert();

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_selection);

    let mut rdr = rconfig.simd_zero_copy_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let headers = rdr.byte_headers()?;
    let sel = rconfig.selection(headers)?;

    if sel.is_empty() {
        Err("cannot drop all the columns!")?;
    }

    if !rconfig.no_headers {
        wtr.write_record(sel.select(headers))?;
    }

    while let Some(record) = rdr.read_byte_record()? {
        wtr.write_record_no_quoting(sel.select(&record))?;
    }

    Ok(wtr.flush()?)
}
