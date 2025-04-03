use std::io::Write;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

use crate::moonblade::AggregationProgram;
use crate::moonblade::GroupAggregationProgram;

use crate::cmd::moonblade::MoonbladeErrorPolicy;

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

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

For a list of available aggregation functions, use `xan help aggs`
instead.

Usage:
    xan groupby [options] <column> <expression> [<input>]
    xan groupby --help

groupby options:
    --keep <cols>           Keep this selection of columns, in addition to
                            the ones representing groups, in the output. Only
                            values from the first seen row per group will be kept.
    -S, --sorted            Use this flag to indicate that the file is already sorted on the
                            group columns, in which case the command will be able to considerably
                            optimize memory usage.
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
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

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
