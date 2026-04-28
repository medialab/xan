use bstr::ByteSlice;
use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::moonblade::{WindowAggregationArray, WindowAggregationProgram};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

#[inline]
fn build_along_columns_record(
    mask: &[bool],
    input_record: &ByteRecord,
    output_record: &mut ByteRecord,
    records: &[ByteRecord],
    overwrite: bool,
) {
    output_record.clear();

    let mut records_iter = records.iter();

    for (i, is_mapped) in mask.iter().copied().enumerate() {
        if is_mapped {
            if !overwrite {
                output_record.push_field(&input_record[i]);
            }

            let current_record = records_iter.next().unwrap();

            for cell in current_record {
                output_record.push_field(cell);
            }
        } else {
            output_record.push_field(&input_record[i]);
        }
    }
}

static USAGE: &str = "
Compute window aggregations such as cumulative sums, rolling means, leading and
lagging values, rankings etc.

This command is able to compute multiple aggregations in a single pass over the
file, and never uses more memory that required to fit the largest desired window
for rolling stats and leads/lags.

Ranking aggregations however (such as `frac` or `dense_rank`), still require to
buffer the whole file in memory (or at least whole groups when using -g/--groupby),
since they cannot be computed otherwise.

Computing a cumulative sum:

    $ xan window 'cumsum(n)' file.csv

Computing a rolling mean & variance:

    $ xan window 'rolling_mean(10, n) as mean, rolling_var(10, n) as var' file.csv

Adding a lagged column:

    $ xan window 'lag(n) as \"n-1\"' file.csv

Ranking numerical values:

    $ xan window 'dense_rank(n) as rank' file.csv

Computing fraction of cell wrt total sum of target column:

    $ xan window 'frac(n) as frac' file.csv

This command is also able to reset the statistics each time a new contiguous group
of rows is encountered using the -g/--groupby flag. This means, however, that
the file must be sorted by columns representing group identities beforehand:

    $ xan window -g country 'cumsum(n)' file.csv

Finally, this command can also run arbitrary aggregation functions (like
with `xan agg` & `xan groupby`) for the whole file or per group and repeat their
result for each row. This can be useful to filter rows belonging to some group
(e.g. if an aggregated score is over some threshold), or for normalization purposes.

Note that when doing so, the whole file will be buffered to memory.

Keeping rows belonging to groups whose average for the `count` column is over 10:

    $ xan window -g country 'mean(count) as mean' file.csv | xan filter 'mean > 10'

# Window aggregationgs along columns

Sometimes you might want to add one or more columns in a same fashion for a given
selection of columns.

You can do so using the -C/--along-columns <cols> flag. In this case, the `_`
placeholder can be used in expression to represent the current column.

For instance, given the following data:

a,b
4,5
1,7

The following command (notice how we can template added column names):

    $ xan window -C a,b 'mean(_) as \"{}_mean\", lag(_) as \"{}_lag\"' file.csv

Would produce the following:

a,a_mean,a_lag,b,b_mean,b_lag
4,2.5,,5,6.0,
1,2.5,4,7,6.0,5

This can also be used with the -O/--overwrite flag:

    $ xan window -OC a,b 'mean(_) as \"{}_mean\", lag(_) as \"{}_lag\"' file.csv

To produce:

a_mean,a_lag,b_mean,b_lag
2.5,,6.0,
2.5,4,6.0,5

---

For a list of available window aggregation functions, use `xan help window`.

For a list of available generic aggregation functions, use `xan help aggs`.

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan window [options] <expression> [<input>]
    xan window --help

window options:
    -g, --groupby <cols>        If given, resets the computed aggregations each
                                time the given selection yields a new identity.
    -O, --overwrite             If set, expressions named with a column already existing
                                in the file will be overwritten with the result of the
                                expression instead of adding a new column at the end.
                                This means you can both transform and add columns at the
                                same time.
    -C, --along-columns <cols>  Repeat same expression over a selection of columns at once.

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
    flag_groupby: Option<SelectedColumns>,
    flag_overwrite: bool,
    flag_along_columns: Option<SelectedColumns>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut reader = conf.simd_reader()?;

    let mut writer = Config::new(&args.flag_output).simd_writer()?;

    let headers = reader.byte_headers()?.clone();
    let mut program =
        WindowAggregationProgram::parse(&args.arg_expression, &headers, conf.no_headers)?;

    if args.flag_along_columns.is_none() && args.flag_overwrite {
        program.overwrite();
    }

    let groupby_sel_opt = args
        .flag_groupby
        .map(|s| s.selection(&headers, !conf.no_headers))
        .transpose()?;

    let columns_sel_opt = args
        .flag_along_columns
        .map(|s| s.selection(&headers, !conf.no_headers))
        .transpose()?;

    let mut array_opt = columns_sel_opt.map(|s| {
        (
            s.mask(headers.len()),
            WindowAggregationArray::from_program(&program, &s),
        )
    });

    if !conf.no_headers {
        if let Some((mask, _)) = &array_opt {
            let mut new_headers = ByteRecord::new();

            for (i, is_mapped) in mask.iter().copied().enumerate() {
                if is_mapped {
                    if !args.flag_overwrite {
                        new_headers.push_field(&headers[i]);
                    }

                    for name in program.headers() {
                        let templated = name.replace("{}", &headers[i]);
                        new_headers.push_field(&templated);
                    }
                } else {
                    new_headers.push_field(&headers[i]);
                }
            }

            writer.write_byte_record(&new_headers)?;
        } else {
            writer.write_record(headers.iter().chain(program.headers()))?;
        }
    }

    let mut record = ByteRecord::new();
    let mut output_record = ByteRecord::new();
    let mut row_index: usize = 0;
    let mut group_opt: Option<ByteRecord> = None;

    while reader.read_byte_record(&mut record)? {
        if let Some(sel) = &groupby_sel_opt {
            match &mut group_opt {
                None => {
                    group_opt = Some(sel.select(&record).collect());
                }
                Some(group) => {
                    let new_group = sel.select(&record).collect();

                    if group != &new_group {
                        if let Some((mask, array)) = array_opt.as_mut() {
                            array.flush_and_clear(
                                headers.len(),
                                row_index,
                                |r, rs| -> CliResult<()> {
                                    build_along_columns_record(
                                        mask,
                                        &r,
                                        &mut output_record,
                                        &rs,
                                        args.flag_overwrite,
                                    );

                                    writer.write_byte_record(&output_record)?;

                                    Ok(())
                                },
                            )?;
                        } else {
                            program.flush_and_clear(
                                row_index,
                                |output_record| -> CliResult<()> {
                                    writer.write_byte_record(&output_record)?;

                                    Ok(())
                                },
                            )?;
                        }

                        *group = new_group;
                    }
                }
            };
        }

        if let Some((mask, array)) = array_opt.as_mut() {
            if let Some((r, rs)) = array.run_with_record(row_index, &record)? {
                build_along_columns_record(mask, &r, &mut output_record, rs, args.flag_overwrite);

                writer.write_byte_record(&output_record)?;
            }
        } else if let Some(output_record) = program.run_with_record(row_index, None, &record)? {
            writer.write_byte_record(&output_record)?;
        }

        row_index += 1;
    }

    if let Some((mask, array)) = array_opt {
        array.flush(row_index, headers.len(), |r, rs| -> CliResult<()> {
            build_along_columns_record(&mask, &r, &mut output_record, &rs, args.flag_overwrite);

            writer.write_byte_record(&output_record)?;

            Ok(())
        })?;
    } else {
        program.flush(row_index, |output_record| -> CliResult<()> {
            writer.write_byte_record(&output_record)?;

            Ok(())
        })?;
    }

    Ok(writer.flush()?)
}
