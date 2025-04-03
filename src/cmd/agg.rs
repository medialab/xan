use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

use crate::moonblade::AggregationProgram;

use crate::cmd::moonblade::MoonbladeErrorPolicy;

// NOTE: what was tried for parallelization:
//   1. Horizontal parallelization (by execution unit of the aggregation planner)
//   2. Vertical parallelization by broadcasting lines to multiple threads
//   3. Chunking vertical parallelization
//   4. Aggregator finalization parallelization (sorting for median, for instance)

static USAGE: &str = "
Aggregate CSV data using a custom aggregation expression. The result of running
the command will be a single row of CSV containing the result of aggregating
the whole file.

You can, for instance, compute the sum of a column:

    $ xan agg 'sum(retweet_count)' file.csv

You can use dynamic expressions to mangle the data before aggregating it:

    $ xan agg 'sum(retweet_count + replies_count)' file.csv

You can perform multiple aggregations at once:

    $ xan agg 'sum(retweet_count), mean(retweet_count), max(replies_count)' file.csv

You can rename the output columns using the 'as' syntax:

    $ xan agg 'sum(n) as sum, max(replies_count) as \"Max Replies\"' file.csv

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

For a list of available aggregation functions, use `xan help aggs`
instead.

Usage:
    xan agg [options] <expression> [<input>]
    xan agg --help

agg options:
    -E, --errors <policy>    What to do with evaluation errors. One of:
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
                             Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_errors: String,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let error_policy = MoonbladeErrorPolicy::try_from_restricted(&args.flag_errors)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let mut program = AggregationProgram::parse(&args.arg_expression, headers)?;

    wtr.write_record(program.headers())?;

    let mut record = csv::ByteRecord::new();
    let mut index: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        program
            .run_with_record(index, &record)
            .or_else(|error| error_policy.handle_row_error(index, error))?;

        index += 1;
    }

    wtr.write_byte_record(&error_policy.handle_error(program.finalize(false))?)?;

    Ok(wtr.flush()?)
}
