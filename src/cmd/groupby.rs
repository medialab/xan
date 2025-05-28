use std::io::Write;
use std::num::NonZeroUsize;

use crate::cmd::moonblade::MoonbladeErrorPolicy;
use crate::cmd::parallel::Args as ParallelArgs;
use crate::config::{Config, Delimiter};
use crate::moonblade::AggregationProgram;
use crate::moonblade::GroupAggregationProgram;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

fn write_group(
    wtr: &mut csv::Writer<Box<dyn Write + Send>>,
    group: &Vec<Vec<u8>>,
    addendum: &csv::ByteRecord,
) -> CliResult<()> {
    let mut record = csv::ByteRecord::new();
    record.extend(group);
    record.extend(addendum);

    wtr.write_byte_record(&record)?;

    Ok(())
}

static USAGE: &str = "
Group a CSV file by values contained in a column selection then aggregate data per
group using a custom aggregation expression.

The result of running the command will be a CSV file containing the grouped
columns and additional columns for each computed aggregation.

You can, for instance, compute the sum of a column per group:

    $ xan groupby user_name 'sum(retweet_count)' file.csv

You can use dynamic expressions to mangle the data before aggregating it:

    $ xan groupby user_name 'sum(retweet_count + replies_count)' file.csv

You can perform multiple aggregations at once:

    $ xan groupby user_name 'sum(retweet_count), mean(retweet_count), max(replies_count)' file.csv

You can rename the output columns using the 'as' syntax:

    $ xan groupby user_name 'sum(n) as sum, max(replies_count) as \"Max Replies\"' file.csv

You can group on multiple columns (read `xan select -h` for more information about column selection):

    $ xan groupby name,surname 'sum(count)' file.csv

For a list of available aggregation functions, use `xan help aggs`
instead.

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Aggregations can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -S/--sorted nor -E/--errors options.

Note that when using parallelization, groups may appear in different order in the
output with each run.

Usage:
    xan groupby [options] <column> <expression> [<input>]
    xan groupby --help

groupby options:
    --keep <cols>            Keep this selection of columns, in addition to
                             the ones representing groups, in the output. Only
                             values from the first seen row per group will be kept.
    -S, --sorted             Use this flag to indicate that the file is already sorted on the
                             group columns, in which case the command will be able to considerably
                             optimize memory usage.
    -e, --errors <policy>    What to do with evaluation errors. One of:
                               - \"panic\": exit on first error
                               - \"ignore\": ignore row altogether
                               - \"log\": print error to stderr
                             [default: panic].
    -p, --parallel           Whether to use parallelization to speed up computation.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

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
    arg_column: SelectColumns,
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_keep: Option<SelectColumns>,
    flag_sorted: bool,
    flag_errors: String,
    flag_parallel: bool,
    flag_threads: Option<NonZeroUsize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_sorted {
            Err("-p/--parallel or -t/--threads cannot be used with --sorted!")?;
        }

        if args.flag_errors != "panic" {
            Err("-p/--parallel or -t/--threads cannot be used with -E/--errors!")?;
        }

        let mut parallel_args = ParallelArgs::single_file(&args.arg_input, args.flag_threads)?;

        parallel_args.cmd_groupby = true;
        parallel_args.arg_group = Some(args.arg_column);
        parallel_args.arg_expr = Some(args.arg_expression);

        return parallel_args.run();
    }

    let error_policy = MoonbladeErrorPolicy::try_from_restricted(&args.flag_errors)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let sel = rconf.selection(headers)?;

    // Lol, what a hack...
    if let Some(selection) = args.flag_keep.take() {
        let mut keep_sel = selection.selection(headers, !args.flag_no_headers)?;
        keep_sel.sort_and_dedup();

        let addendum = keep_sel
            .iter()
            .filter(|i| !sel.contains(**i))
            .copied()
            .map(|i| {
                format!(
                    "first(col({})) as \"{}\"",
                    i,
                    std::str::from_utf8(&headers[i]).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(", ");

        if !addendum.is_empty() {
            args.arg_expression = addendum + ", " + &args.arg_expression;
        }
    }

    let mut record = csv::ByteRecord::new();

    if args.flag_sorted {
        let mut program = AggregationProgram::parse(&args.arg_expression, headers)?;
        let mut current: Option<Vec<Vec<u8>>> = None;

        write_group(
            &mut wtr,
            &sel.collect(headers),
            &program.headers().collect(),
        )?;

        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            match current.as_ref() {
                None => {
                    current = Some(group);
                }
                Some(current_group) => {
                    if current_group != &group {
                        write_group(
                            &mut wtr,
                            current_group,
                            &error_policy.handle_error(program.finalize(false))?,
                        )?;
                        program.clear();
                        current = Some(group);
                    }
                }
            };

            program
                .run_with_record(index, &record)
                .or_else(|error| error_policy.handle_row_error(index, error))?;

            index += 1;
        }

        // Flushing final group
        if let Some(current_group) = current {
            write_group(
                &mut wtr,
                &current_group,
                &error_policy.handle_error(program.finalize(false))?,
            )?;
        }
    } else {
        let mut program = GroupAggregationProgram::parse(&args.arg_expression, headers)?;

        write_group(
            &mut wtr,
            &sel.collect(headers),
            &program.headers().collect(),
        )?;

        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            program
                .run_with_record(group, index, &record)
                .or_else(|error| error_policy.handle_row_error(index, error))?;

            index += 1;
        }

        for result in program.into_byte_records(false) {
            let (group, group_record) = error_policy.handle_error(result)?;

            write_group(&mut wtr, &group, &group_record)?;
        }
    }

    Ok(wtr.flush()?)
}
