use std::io::Write;
use std::num::NonZeroUsize;

use crate::cmd::parallel::Args as ParallelArgs;
use crate::config::{Config, Delimiter};
use crate::moonblade::{
    AggregationProgram, GroupAggregationProgram, GroupBroadcastAggregationProgram,
};
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

---

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

    $ xan groupby user --along-cols count1,count2 'sum(cell)' file.csv

Will produce the following result:

user,count1,count2
marcy,10,13
john,4,7

---

For a list of available aggregation functions, use `xan help aggs`
instead.

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Aggregations can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -S/--sorted nor -C/--along-cols flags.

Usage:
    xan groupby [options] <column> <expression> [<input>]
    xan groupby --help

groupby options:
    --keep <cols>            Keep this selection of columns, in addition to
                             the ones representing groups, in the output. Only
                             values from the first seen row per group will be kept.
    -C, --along-cols <cols>  Perform a single aggregation over all of selected columns
                             and create a column per group with the result in the output.
    -S, --sorted             Use this flag to indicate that the file is already sorted on the
                             group columns, in which case the command will be able to considerably
                             optimize memory usage.
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
    flag_along_cols: Option<SelectColumns>,
    flag_sorted: bool,
    flag_parallel: bool,
    flag_threads: Option<NonZeroUsize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_along_cols.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with --along-cols!")?;
        }

        if args.flag_sorted {
            Err("-p/--parallel or -t/--threads cannot be used with --sorted!")?;
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

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let sel = rconf.selection(headers)?;

    // --along-cols
    if let Some(selection) = args.flag_along_cols.take() {
        if args.flag_sorted || args.flag_keep.is_some() {
            Err("--along-cols does not work with -S/--sorted nor --keep!")?;
        }

        let mut pivot_sel = selection.selection(headers, !args.flag_no_headers)?;
        pivot_sel.sort_and_dedup();

        let mut program = GroupBroadcastAggregationProgram::parse(
            &args.arg_expression,
            headers,
            pivot_sel.len(),
        )?;

        if !args.flag_no_headers {
            let mut output_headers = sel.select(headers).collect::<csv::ByteRecord>();

            for name in pivot_sel.select(headers) {
                output_headers.push_field(name);
            }

            wtr.write_byte_record(&output_headers)?;
        }

        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            program.run_with_cells(group, index, &record, pivot_sel.select(&record))?;

            index += 1;
        }

        for result in program.into_byte_records(false) {
            let (group, group_record) = result?;

            write_group(&mut wtr, &group, &group_record)?;
        }

        return Ok(wtr.flush()?);
    }

    // --keep, lol...
    if let Some(selection) = args.flag_keep.take() {
        let mut keep_sel = selection.selection(headers, !args.flag_no_headers)?;
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

    let mut record = csv::ByteRecord::new();

    if args.flag_sorted {
        let mut program = AggregationProgram::parse(&args.arg_expression, headers)?;
        let mut current: Option<Vec<Vec<u8>>> = None;

        if !args.flag_no_headers {
            write_group(
                &mut wtr,
                &sel.collect(headers),
                &program.headers().collect(),
            )?;
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
                        write_group(&mut wtr, current_group, &program.finalize(false)?)?;
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
            write_group(&mut wtr, &current_group, &program.finalize(false)?)?;
        }
    } else {
        let mut program = GroupAggregationProgram::parse(&args.arg_expression, headers)?;

        if !args.flag_no_headers {
            write_group(
                &mut wtr,
                &sel.collect(headers),
                &program.headers().collect(),
            )?;
        }

        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let group = sel.collect(&record);

            program.run_with_record(group, index, &record)?;

            index += 1;
        }

        for result in program.into_byte_records(false) {
            let (group, group_record) = result?;

            write_group(&mut wtr, &group, &group_record)?;
        }
    }

    Ok(wtr.flush()?)
}
