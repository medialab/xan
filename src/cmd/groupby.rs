use std::num::NonZeroUsize;

use crate::cmd::parallel::Args as ParallelArgs;
use crate::config::{Config, Delimiter};
use crate::moonblade::{
    AggregationProgram, GroupAggregationProgram, GroupAlongColumnsAggregationProgram,
};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Group a CSV file by values contained in a column selection then aggregate data per
group using a custom aggregation expression.

For ungrouped aggregation, check the `xan agg` command instead.

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

# Computing a total aggregate in the same pass

This command can compute a total aggregate over the whole file in the same pass
as computing aggregates per group so you can easily pipe into other commands to
compute ratios and such.

For instance, given the following file:

user,count
marcy,5
john,2
marcy,6
john,4

Using the following command:

    $ xan groupby user 'sum(count) as count' -T 'sum(count) as total' file.csv

Will produce the following result:

user,count,total
john,7,17
marcy,10,17

You can then pipe this into e.g. `xan select -e` and get a ratio:

    $ <command-above> | xan select -e 'user, count, (count / total).to_fixed(2) as ratio'

To produce:

user,count,ratio
marcy,11,0.65
john,6,0.35

# Aggregating along columns

This command is also able to aggregate along columns that you can select using
the --along-cols <cols> flag. In which case, the aggregation functions will accept
the anonymous `_` placeholder representing currently processed column's value.

For instance, given the following file:

user,count1,count2
marcy,4,5
john,0,1
marcy,6,8
john,4,6

Using the following command:

    $ xan groupby user --along-cols count1,count2 'sum(_)' file.csv

Will produce the following result:

user,count1,count2
marcy,10,13
john,4,7

# Aggregating along matrix

This command can also aggregate over all values of a selection of columns, thus
representing a 2-dimensional matrix, using the -M/--along-matrix flag. In which
case aggregation functions will accept the anonymous `_` placeholder value representing
the currently processed column's value.

For instance, given the following file:

user,count1,count2
marcy,4,5
john,0,1
marcy,6,8
john,4,6

Using the following command:

    $ xan groupby user --along-matrix count1,count2 'sum(_) as total' file.csv

Will produce the following result:

user,total
marcy,23
john,11

---

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available aggregation functions, use `xan help aggs`.

For a list of available functions, use `xan help functions`.

Aggregations can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -S/--sorted nor -C/--along-cols flags.

Usage:
    xan groupby [options] <column> <expression> [<input>]
    xan groupby --help

groupby options:
    --keep <cols>              Keep this selection of columns, in addition to
                               the ones representing groups, in the output. Only
                               values from the first seen row per group will be kept.
    -C, --along-cols <cols>    Perform a single aggregation over all of selected columns
                               and create a column per group with the result in the output.
    -M, --along-matrix <cols>  Aggregate all values found in the given selection
                               of columns.
    -T, --total <expr>         Run an aggregation over the whole file in the same pass over
                               the data and add the resulting columns at the end of each group's
                               result. Can be useful to compute ratios over total etc in a single
                               pass when piping into `map`, `transform`, `select -e` etc.
    -S, --sorted               Use this flag to indicate that the file is already sorted on the
                               group columns, in which case the command will be able to considerably
                               optimize memory usage.
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
    arg_column: SelectColumns,
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_keep: Option<SelectColumns>,
    flag_along_cols: Option<SelectColumns>,
    flag_along_matrix: Option<SelectColumns>,
    flag_total: Option<String>,
    flag_sorted: bool,
    flag_parallel: bool,
    flag_threads: Option<NonZeroUsize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_along_cols.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -C/--along-cols!")?;
        }

        if args.flag_along_matrix.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -M/--along-matrix!")?;
        }

        if args.flag_sorted {
            Err("-p/--parallel or -t/--threads cannot be used with -S/--sorted!")?;
        }

        if args.flag_total.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -T/--total!")?;
        }

        let mut parallel_args = ParallelArgs::single_file(&args.arg_input, args.flag_threads)?;

        parallel_args.cmd_groupby = true;
        parallel_args.arg_group = Some(args.arg_column);
        parallel_args.arg_expr = Some(args.arg_expression);

        parallel_args.flag_no_headers = args.flag_no_headers;
        parallel_args.flag_output = args.flag_output;
        parallel_args.flag_delimiter = args.flag_delimiter;

        return parallel_args.run();
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut rdr = rconf.simd_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;
    let headers = rdr.byte_headers()?;

    let sel = rconf.selection(headers)?;

    let mut total_program_opt = args
        .flag_total
        .as_ref()
        .map(|total| AggregationProgram::parse(total, headers))
        .transpose()?;

    // --along-cols
    if let Some(selection) = args.flag_along_cols.take() {
        if args.flag_sorted || args.flag_keep.is_some() {
            Err("-C/--along-cols does not work with -S/--sorted nor --keep!")?;
        }

        if args.flag_total.is_some() {
            Err("-T/--total does work yet with -C/--along-cols!")?;
        }

        let mut pivot_sel = selection.selection(headers, !rconf.no_headers)?;
        pivot_sel.sort_and_dedup();

        let mut program = GroupAlongColumnsAggregationProgram::parse(
            &args.arg_expression,
            headers,
            pivot_sel.len(),
        )?;

        if !rconf.no_headers {
            let mut output_headers = sel.select(headers).collect::<simd_csv::ByteRecord>();

            for name in pivot_sel.select(headers) {
                output_headers.push_field(name);
            }

            wtr.write_byte_record(&output_headers)?;
        }

        let mut record = simd_csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            program.run_with_cells(group, index, &record, pivot_sel.select(&record))?;

            index += 1;
        }

        for result in program.into_byte_records(false) {
            let (group, group_record) = result?;

            wtr.write_record(
                group
                    .iter()
                    .map(|cell| cell.as_slice())
                    .chain(group_record.iter()),
            )?;
        }

        return Ok(wtr.flush()?);
    }

    // --along-matrix
    if let Some(selection) = args.flag_along_matrix.take() {
        if args.flag_sorted || args.flag_keep.is_some() {
            Err("--along-matrix does not work with -S/--sorted nor --keep!")?;
        }

        if args.flag_total.is_some() {
            Err("-T/--total does work yet with -M/--along-matrix!")?;
        }

        let mut matrix_sel = selection.selection(headers, !rconf.no_headers)?;
        matrix_sel.sort_and_dedup();

        let mut program =
            GroupAggregationProgram::<Vec<Vec<u8>>>::parse(&args.arg_expression, headers)?;

        if !rconf.no_headers {
            wtr.write_record(sel.select(headers).chain(program.headers()))?;
        }

        let mut record = simd_csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            program.run_with_cells(group, index, &record, matrix_sel.select(&record))?;

            index += 1;
        }

        for result in program.into_byte_records(false) {
            let (group, group_record) = result?;

            wtr.write_record(
                group
                    .iter()
                    .map(|cell| cell.as_slice())
                    .chain(group_record.iter()),
            )?;
        }

        return Ok(wtr.flush()?);
    }

    // --keep, lol...
    if let Some(selection) = args.flag_keep.take() {
        let mut keep_sel = selection.selection(headers, !rconf.no_headers)?;
        keep_sel.dedup();

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

    let mut record = simd_csv::ByteRecord::new();

    if args.flag_sorted {
        if args.flag_total.is_some() {
            Err("-T/--total cannot work with -S/--sorted!")?;
        }

        let mut program = AggregationProgram::parse(&args.arg_expression, headers)?;
        let mut current: Option<Vec<Vec<u8>>> = None;

        if !rconf.no_headers {
            wtr.write_record(sel.select(headers).chain(program.headers()))?;
        }

        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            match current.as_ref() {
                None => {
                    current = Some(group);
                }
                Some(current_group) => {
                    if current_group != &group {
                        wtr.write_record(
                            current_group
                                .iter()
                                .map(|cell| cell.as_slice())
                                .chain(&program.finalize(false)?),
                        )?;

                        program.clear();
                        current = Some(group);
                    }
                }
            };

            program.run_with_record(index, &record)?;

            index += 1;
        }

        // Flushing final group
        if let Some(current_group) = current {
            wtr.write_record(
                current_group
                    .iter()
                    .map(|cell| cell.as_slice())
                    .chain(&program.finalize(false)?),
            )?;
        }
    } else {
        let mut program = GroupAggregationProgram::parse(&args.arg_expression, headers)?;

        if !rconf.no_headers {
            if let Some(total_program) = &total_program_opt {
                wtr.write_record(
                    sel.select(headers)
                        .chain(program.headers())
                        .chain(total_program.headers()),
                )?;
            } else {
                wtr.write_record(sel.select(headers).chain(program.headers()))?;
            }
        }

        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            program.run_with_record(group, index, &record)?;

            if let Some(total_program) = total_program_opt.as_mut() {
                total_program.run_with_record(index, &record)?;
            }

            index += 1;
        }

        let total_record_opt = total_program_opt
            .map(|mut total_program| total_program.finalize(false))
            .transpose()?;

        for result in program.into_byte_records(false) {
            let (group, group_record) = result?;

            if let Some(total_record) = &total_record_opt {
                wtr.write_record(
                    group
                        .iter()
                        .map(|cell| cell.as_slice())
                        .chain(group_record.iter())
                        .chain(total_record.iter()),
                )?;
            } else {
                wtr.write_record(
                    group
                        .iter()
                        .map(|cell| cell.as_slice())
                        .chain(group_record.iter()),
                )?;
            }
        }
    }

    Ok(wtr.flush()?)
}
