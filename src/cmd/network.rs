// use std::rc::Rc;

// use crate::config::Delimiter;
// use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

// struct Node {
//     key: Rc<String>,
// }

// struct Edge {
//     source: Rc<String>,
//     target: Rc<String>,
//     undirected: bool,
// }

static USAGE: &str = "
TODO...

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network --help

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    // arg_input: Option<String>,
    // arg_source: Option<SelectColumns>,
    // arg_target: Option<SelectColumns>,
    // flag_no_headers: bool,
    // flag_delimiter: Option<Delimiter>,
    // flag_output: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    // let rconf = Config::new(&args.arg_input)
    //     .delimiter(args.flag_delimiter)
    //     .no_headers(args.flag_no_headers);

    dbg!(&args);

    Ok(())
}
