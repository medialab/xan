use std::convert::TryFrom;

use crate::cmd::moonblade::{
    run_moonblade_cmd, MoonbladeCmdArgs, MoonbladeErrorPolicy, MoonbladeMode,
};
use crate::config::Delimiter;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
The transform command evaluates an expression for each row of the given CSV file
and use the result to edit a target column that can optionally be renamed.

For instance, given the following CSV file:

name,surname
john,davis
mary,sue

The following command:

    $ xan transform surname 'upper(surname)'

Will produce the following result:

name,surname
john,DAVIS
mary,SUE

Note that the given expression will be given the target column as its implicit
value, which means that the latter command can also be written as:

    $ xan transform surname 'upper'

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan transform [options] <column> <expression> [<input>]
    xan transform --help

transform options:
    -r, --rename <name>        New name for the transformed column.
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.
    -E, --errors <policy>      What to do with evaluation errors. One of:
                                 - "panic": exit on first error
                                 - "report": add a column containing error
                                 - "ignore": coerce result for row to null
                                 - "log": print error to stderr
                               [default: panic].
    --error-column <name>      Name of the column containing errors if
                               "-E/--errors" is set to "report".
                               [default: xan_error].

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
    arg_column: String,
    arg_expression: String,
    arg_input: Option<String>,
    flag_rename: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_errors: String,
    flag_error_column: String,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let moonblade_args = MoonbladeCmdArgs {
        target_column: Some(args.arg_column),
        rename_column: args.flag_rename,
        map_expr: args.arg_expression,
        input: args.arg_input,
        output: args.flag_output,
        no_headers: args.flag_no_headers,
        delimiter: args.flag_delimiter,
        parallelization,
        error_policy: MoonbladeErrorPolicy::try_from(args.flag_errors)?,
        error_column_name: Some(args.flag_error_column),
        mode: MoonbladeMode::Transform,
        ..Default::default()
    };

    run_moonblade_cmd(moonblade_args)
}
