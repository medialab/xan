use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::sync::Arc;

use csv;
use rayon::prelude::*;
use thread_local::ThreadLocal;

use config::{Config, Delimiter};
use util::{self, ChunksIteratorExt};
use CliResult;

use moonblade::AggregationProgram;

use cmd::moonblade::{
    get_moonblade_aggregations_function_help, get_moonblade_cheatsheet,
    get_moonblade_functions_help, MoonbladeErrorPolicy,
};

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

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

For a list of available aggregation functions, use the --aggs flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan agg [options] <expression> [<input>]
    xan agg --help
    xan agg --cheatsheet
    xan agg --aggs
    xan agg --functions

agg options:
    -e, --errors <policy>    What to do with evaluation errors. One of:
                               - \"panic\": exit on first error
                               - \"ignore\": ignore row altogether
                               - \"log\": print error to stderr
                             [default: panic].
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores.
    -c, --chunk-size <size>  Number of rows in a batch to send to a thread at once when
                             using -p, --parallel.
                             [default: 4096]

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
    flag_parallel: bool,
    flag_chunk_size: NonZeroUsize,
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

    wtr.write_record(program.headers())?;

    if !args.flag_parallel {
        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            index += 1;

            program
                .run_with_record(index, &record)
                .or_else(|error| error_policy.handle_row_error(index, error))?;
        }
    } else {
        // NOTE: it looks like parallelization is basically moot if the inner
        // expressions are trivial. Reading the CSV file linearly is the bottleneck here.
        // This means that if what is parallelized runs faster than actually reading
        // the CSV file, parallelization does not yield any performance increase.
        // This can somehow be tweaked by sending chunks, but not that much.
        // So if you read files or perform costly computations for each row, it might be
        // worthwhile. Else it will actually hurt performance...
        let local: Arc<ThreadLocal<RefCell<AggregationProgram>>> = Arc::new(ThreadLocal::new());

        rdr.into_byte_records()
            .enumerate()
            .chunks(args.flag_chunk_size)
            .par_bridge()
            .try_for_each(|chunk| -> CliResult<()> {
                for (index, rdr_result) in chunk {
                    let record = rdr_result?;

                    let mut local_program =
                        local.get_or(|| RefCell::new(program.clone())).borrow_mut();

                    local_program
                        .run_with_record(index, &record)
                        .or_else(|error| error_policy.handle_row_error(index, error))?;
                }

                Ok(())
            })?;

        for local_program in Arc::try_unwrap(local).unwrap().into_iter() {
            program.merge(local_program.into_inner());
        }
    }

    wtr.write_byte_record(&error_policy.handle_error(program.finalize(args.flag_parallel))?)?;

    Ok(wtr.flush()?)
}
