use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

use crate::moonblade::SelectionProgram;

use crate::cmd::moonblade::{
    get_moonblade_cheatsheet, get_moonblade_functions_help, MoonbladeErrorPolicy,
};

static USAGE: &str = "
Select columns from CSV data efficiently using either a handy DSL or by
evaluating an expression on each row (using the -e, --evaluate flag).

This command lets you manipulate the columns in CSV data. You can re-order
them, duplicate them, transform them or drop them.

1. Selection DSL:
-----------------

Columns can be referenced by index or byname if there is a header row (duplicate
column names can be disambiguated with more indexing).

Finally, column ranges can be specified.

  Select the first and fourth columns:
    $ xan select 0,3

  Select the first 4 columns (by index and by name):
    $ xan select 0-3
    $ xan select Header1-Header4

  Ignore the first 2 columns (by range and by omission):
    $ xan select 2-
    $ xan select '!0-1'

  Select the third column named 'Foo':
    $ xan select 'Foo[2]'

  Re-order and duplicate columns arbitrarily:
    $ xan select 3-1,Header3-Header1,Header1,Foo[2],Header1

  Quote column names that conflict with selector syntax:
    $ xan select '\"Date - Opening\",\"Date - Actual Closing\"'

  Select all the columns (useful to add some copies of columns):
    $ xan select '*'
    $ xan select '*,name'
    $ xan select '*,1'
    $ xan select '0-'
    $ xan select '-0'

2. Evaluating a expression:
---------------------------

Using a SQLish syntax that is the same as for the `map`, `agg`, `filter` etc.
commands, you can wrangle the rows and perform a custom selection.

  $ xan select -e 'name, prenom as surname, count1 + count2 as total'

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

For a list of available aggregation functions, use the --aggs flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan select [options] [--] <selection> [<input>]
    xan select --help
    xan select --cheatsheet
    xan select --functions

select options:
    -e, --evaluate           Toggle expression evaluation rather than using the DSL.
    -E, --errors <policy>    What to do with evaluation errors. One of:
                               - \"panic\": exit on first error
                               - \"ignore\": ignore row altogether
                               - \"log\": print error to stderr
                             [default: panic].

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
    arg_selection: String,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_cheatsheet: bool,
    flag_functions: bool,
    flag_evaluate: bool,
    flag_errors: String,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_cheatsheet {
        println!("{}", get_moonblade_cheatsheet());
        return Ok(());
    }

    if args.flag_functions {
        println!("{}", get_moonblade_functions_help());
        return Ok(());
    }

    let mut rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);
    // .select(args.arg_selection);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let mut record = csv::ByteRecord::new();

    let headers = rdr.byte_headers()?.clone();

    if !args.flag_evaluate {
        let parsed_selection = SelectColumns::parse(&args.arg_selection)?;
        rconfig = rconfig.select(parsed_selection);

        let sel = rconfig.selection(&headers)?;

        if !rconfig.no_headers {
            wtr.write_record(sel.iter().map(|&i| &headers[i]))?;
        }

        while rdr.read_byte_record(&mut record)? {
            wtr.write_record(sel.iter().map(|&i| &record[i]))?;
        }
    } else {
        let error_policy = MoonbladeErrorPolicy::try_from_restricted(&args.flag_errors)?;

        let program = SelectionProgram::parse(&args.arg_selection, &headers)?;

        wtr.write_record(program.headers())?;

        let index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let output_record =
                error_policy.handle_error(program.run_with_record(index, &record))?;

            wtr.write_byte_record(&output_record)?;
        }
    }

    Ok(wtr.flush()?)
}
