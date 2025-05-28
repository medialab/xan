use crate::cmd::slice::Args as SliceArgs;
use crate::config::Delimiter;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Return the last rows of a CSV file.

An alias for `xan slice -L/--last <n>`.

Usage:
    xan tail [options] [<input>]

head options:
    --rows <n>  Number of rows to return. [default: 10]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Otherwise, the first row will always
                           appear in the output as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_rows: usize,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let mut slice_args = SliceArgs::default();
    slice_args.arg_input = args.arg_input;
    slice_args.flag_last = Some(args.flag_rows);
    slice_args.flag_output = args.flag_output;
    slice_args.flag_no_headers = args.flag_no_headers;
    slice_args.flag_delimiter = args.flag_delimiter;

    slice_args.run()
}
