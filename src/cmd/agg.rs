use csv;

use config::{Config, Delimiter};
use util;
use CliResult;

use moonblade::AggregationProgram;

use cmd::moonblade::{
    get_moonblade_aggregations_function_help, get_moonblade_cheatsheet,
    get_moonblade_functions_help, MoonbladeErrorPolicy,
};

static USAGE: &str = "
Aggregate CSV data using a custom aggregation expression. The result of running
the command will be a single row of CSV containing the result of aggregating
the whole file.

You can, for instance, compute the sum of a column:

    $ xsv agg 'sum(retweet_count)' > result.csv

You can use dynamic expressions to mangle the data before aggregating it:

    $ xsv agg 'sum(add(retweet_count, replies_count))' > result.csv

You can perform multiple aggregations at once:

    $ xsv agg 'sum(retweet_count), mean(retweet_count), max(replies_count)' > result.csv

You can rename the output columns using the 'as' syntax:

    $ xsv agg 'sum(n) as sum, max(replies_count) as \"Max Replies\"' > result.csv

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

For a list of available aggregation functions, use the --aggs flag.

If you want to list available functions, use the --functions flag.

Usage:
    xsv agg [options] <expression> [<input>]
    xsv agg --help
    xsv agg --cheatsheet
    xsv agg --aggs
    xsv agg --functions

agg options:
    -e, --errors <policy>   What to do with evaluation errors. One of:
                              - \"panic\": exit on first error
                              - \"ignore\": ignore row altogether
                              - \"log\": print error to stderr
                            [default: panic].

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_aggs: bool,
    flag_errors: String,
    flag_cheatsheet: bool,
    flag_functions: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_aggs {
        println!("{}", get_moonblade_aggregations_function_help());
        return Ok(());
    }

    if args.flag_cheatsheet {
        println!("{}", get_moonblade_cheatsheet());
        return Ok(());
    }

    if args.flag_functions {
        println!("{}", get_moonblade_functions_help());
        return Ok(());
    }

    let error_policy = MoonbladeErrorPolicy::from_restricted(&args.flag_errors)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let mut program = AggregationProgram::parse(&args.arg_expression, headers)?;

    let mut record = csv::ByteRecord::new();

    wtr.write_byte_record(&program.headers())?;

    let mut index: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        index += 1;

        match program.run_with_record(&record) {
            Ok(_) => (),
            Err(error) => match error_policy {
                MoonbladeErrorPolicy::Panic => Err(error)?,
                MoonbladeErrorPolicy::Ignore => continue,
                MoonbladeErrorPolicy::Log => {
                    eprintln!("Row n°{}: {}", index, error);
                    continue;
                }
                _ => unreachable!(),
            },
        };
    }

    wtr.write_byte_record(&program.finalize())?;

    Ok(wtr.flush()?)
}
