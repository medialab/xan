use cmd::moonblade::{run_moonblade_cmd, MoonbladeCmdArgs, MoonbladeErrorPolicy, MoonbladeMode};
use config::Delimiter;
use util;
use CliResult;

static USAGE: &str = r#"
The flatmap command evaluates an expression for each row of the given CSV file.
This expression is expected to return a potentially iterable value (e.g. a list).

If said value is falsey, then no row will be written in the output of the input
row.

Then, for each nested value yielded by the expression, one row of CSV will be
written to the output.

This row will have the same columns as the input with an additional one
containing the nested value or replacing the value of a column of your choice,
using the -r/--replace flag.

For instance, given the following CSV file:

name,colors
John,blue
Mary,yellow|red

The following command:

    $ xan flatmap 'split(colors, "|")' color -r colors

Will produce the following result:

name,color
John,blue
Mary,yellow
Mary,red

Note that if the expression returns an empty list or a falsey value, no row will
be written in the output for the current input row. This means one can use the
flatmap command as a sort of combined map and filter in a single pass over the CSV file.

For instance, given the following CSV file:

name,age
John Mayer,34
Mary Sue,45

The following command:

    $ xan flatmap 'if(gte(age, 40), last(split(name, " ")))' surname

Will produce the following result:

name,age,surname
Mary Sue,45,Sue

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan flatmap [options] <expression> <column> [<input>]
    xan flatmap --cheatsheet
    xan flatmap --functions
    xan flatmap --help

flatmap options:
    -r, --replace <column>     Name of a column to replaced with the mapped value.
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.
    -e, --errors <policy>      What to do with evaluation errors. One of:
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
                             Must be a single character. [default: ,]
"#;

#[derive(Deserialize)]
struct Args {
    arg_column: String,
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
    flag_replace: Option<String>,
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
        target_column: Some(args.arg_column),
        rename_column: args.flag_replace,
        map_expr: args.arg_expression,
        input: args.arg_input,
        output: args.flag_output,
        no_headers: args.flag_no_headers,
        delimiter: args.flag_delimiter,
        parallelization,
        error_policy: MoonbladeErrorPolicy::from_restricted(&args.flag_errors)?,
        error_column_name: None,
        mode: MoonbladeMode::Flatmap,
    };

    run_moonblade_cmd(moonblade_args)
}
