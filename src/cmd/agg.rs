use std::num::NonZeroUsize;

use crate::cmd::parallel::Args as ParallelArgs;
use crate::config::{Config, Delimiter};
use crate::moonblade::AggregationProgram;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

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

This command can also be used to aggregate a selection of columns per row,
instead of aggregating the whole file, when using the --along-rows flag. In
which case aggregation functions will accept the anonymous `_` placeholder value
representing the currently processed column's value.

Note that when using --along-rows, the `index()` function will return the
index of currently processed column, not the row index. This can be useful
when used with `argmin/argmax` etc.

For instance, given the following CSV file:

name,count1,count2
john,3,6
lucy,10,7

Running the following command (notice the `_` in expression):

    $ xan agg --along-rows count1,count2 'sum(_) as sum'

Will produce the following output:

name,count1,count2,sum
john,3,6,9
lucy,10,7,17

For a list of available aggregation functions, use `xan help aggs`
instead.

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Aggregations can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -R/--along-rows, -M/--along-matrix nor -C/--along-cols options.

Usage:
    xan agg [options] <expression> [<input>]
    xan agg --help

agg options:
    -R, --along-rows <cols>    Aggregate a selection of columns for each row
                               instead of the whole file.
    -C, --along-cols <cols>    Aggregate a selection of columns the same way and
                               return an aggregated column with same name in the
                               output.
    -M, --along-matrix <cols>  Aggregate all values found in the given selection
                               of columns.
    -p, --parallel             Whether to use parallelization to speed up computation.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
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
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_along_rows: Option<SelectColumns>,
    flag_along_cols: Option<SelectColumns>,
    flag_along_matrix: Option<SelectColumns>,
    flag_parallel: bool,
    flag_threads: Option<NonZeroUsize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let agg_modes = args.flag_along_cols.is_some() as u8
        + args.flag_along_rows.is_some() as u8
        + args.flag_along_matrix.is_some() as u8;

    if agg_modes > 1 {
        Err("must select only one of -C/--along-cols & -R/--along-rows!")?;
    }

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_along_rows.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -C/--along-cols!")?;
        }

        if args.flag_along_cols.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -R/--along-rows!")?;
        }

        if args.flag_along_matrix.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -M/--along-matrix!")?;
        }

        let mut parallel_args = ParallelArgs::single_file(&args.arg_input, args.flag_threads)?;

        parallel_args.cmd_agg = true;
        parallel_args.arg_expr = Some(args.arg_expression);

        parallel_args.flag_no_headers = args.flag_no_headers;
        parallel_args.flag_output = args.flag_output;
        parallel_args.flag_delimiter = args.flag_delimiter;

        return parallel_args.run();
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let mut program = AggregationProgram::parse(&args.arg_expression, headers)?;

    // --along-rows
    if let Some(cols) = &args.flag_along_rows {
        let sel = cols.selection(headers, !args.flag_no_headers)?;

        if !args.flag_no_headers {
            wtr.write_record(headers.iter().chain(program.headers()))?;
        }

        let mut record = csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            program.clear();

            for (cell, index) in sel.select(&record).zip(sel.iter().copied()) {
                program.run_with_cell(index, &record, cell)?;
            }

            record.extend(program.finalize(false)?.into_iter());

            wtr.write_record(&record)?;
        }
    }
    // --along-cols
    else if let Some(cols) = &args.flag_along_cols {
        let mut sel = cols.selection(headers, !args.flag_no_headers)?;
        sel.dedup();

        if !program.has_single_expr() {
            Err("expected a single aggregation clause!")?;
        }

        let mut record = csv::ByteRecord::new();

        if !args.flag_no_headers {
            for name in sel.select(headers) {
                record.push_field(name);
            }

            wtr.write_byte_record(&record)?;
        }

        let mut programs = sel.iter().map(|_| program.clone()).collect::<Vec<_>>();

        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            for (cell, p) in sel.select(&record).zip(programs.iter_mut()) {
                p.run_with_cell(index, &record, cell)?;
            }

            index += 1;
        }

        record.clear();

        for p in programs.iter_mut() {
            record.push_field(&p.finalize(false)?[0]);
        }

        wtr.write_byte_record(&record)?;
    }
    // --along-matrix
    else if let Some(cols) = &args.flag_along_matrix {
        let sel = cols.selection(headers, !args.flag_no_headers)?;

        if !args.flag_no_headers {
            wtr.write_record(program.headers())?;
        }

        let mut record = csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            for (cell, index) in sel.select(&record).zip(sel.iter().copied()) {
                program.run_with_cell(index, &record, cell)?;
            }
        }

        wtr.write_byte_record(&program.finalize(false)?)?;
    }
    // Regular
    else {
        // NOTE: we always write headers, because we basically emit a new file
        wtr.write_record(program.headers())?;

        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            program.run_with_record(index, &record)?;

            index += 1;
        }

        wtr.write_byte_record(&program.finalize(false)?)?;
    }
    Ok(wtr.flush()?)
}
