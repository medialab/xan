use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

Usage:
    xan pivot [options] <columns> <name> <value> [<input>]
    xan pivot --help

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter);

    let _ = rconf.reader()?;

    let mut wtr = Config::new(&args.flag_output).writer()?;

    Ok(wtr.flush()?)
}
