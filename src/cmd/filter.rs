use cmd::moonblade::{run_moonblade_cmd, MoonbladeCmdArgs, MoonbladeErrorPolicy, MoonbladeMode};
use config::Delimiter;
use util;
use CliResult;

static USAGE: &str = r#"
The filter command evaluates an expression for each row of the given CSV file and
only output the row if the result of beforementioned expression is truthy.

For instance, given the following CSV file:

a
1
2
3

The following command:

    $ xan filter 'a > 1'

Will produce the following result:

a
2
3

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan filter [options] <expression> [<input>]
    xan filter --cheatsheet
    xan filter --functions
    xan filter --help

filter options:
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.
    -v, --invert-match         If set, will invert the evaluated value.
    -E, --errors <policy>      What to do with evaluation errors. One of:
                                 - "panic": exit on first error
                                 - "ignore": coerce result for row to null
                                 - "log": print error to stderr
                               [default: panic].

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
    arg_expression: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_cheatsheet: bool,
    flag_functions: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_errors: String,
    flag_invert_match: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let moonblade_args = MoonbladeCmdArgs {
        print_cheatsheet: args.flag_cheatsheet,
        print_functions: args.flag_functions,
        target_column: None,
        rename_column: None,
        map_expr: args.arg_expression,
        input: args.arg_input,
        output: args.flag_output,
        no_headers: args.flag_no_headers,
        delimiter: args.flag_delimiter,
        parallelization,
        error_policy: MoonbladeErrorPolicy::from_restricted(&args.flag_errors)?,
        error_column_name: None,
        mode: MoonbladeMode::Filter(args.flag_invert_match),
    };

    run_moonblade_cmd(moonblade_args)
}
