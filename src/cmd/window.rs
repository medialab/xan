use crate::config::{Config, Delimiter};
use crate::moonblade::WindowAggregationProgram;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

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

For a list of available window aggregation functions, use `xan help window`
instead.

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan window [options] <expression> [<input>]
    xan window --help

window options:
    -g, --groupby <cols>  If given, resets the computed aggregations each
                          time the given selection yields a new identity.

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
    flag_groupby: Option<SelectColumns>,
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
    let mut program = WindowAggregationProgram::parse(&args.arg_expression, &headers)?;

    let groupby_sel_opt = args
        .flag_groupby
        .map(|s| s.selection(&headers, !args.flag_no_headers))
        .transpose()?;

    if !args.flag_no_headers {
        writer.write_record(headers.iter().chain(program.headers()))?;
    }

    let mut record = simd_csv::ByteRecord::new();
    let mut index: usize = 0;
    let mut group_opt: Option<Vec<Vec<u8>>> = None;

    while reader.read_byte_record(&mut record)? {
        if let Some(sel) = &groupby_sel_opt {
            match &mut group_opt {
                None => {
                    group_opt = Some(sel.collect(&record));
                }
                Some(group) => {
                    let new_group = sel.collect(&record);

                    if group != &new_group {
                        program.flush_and_clear(index, |output_record| -> CliResult<()> {
                            writer.write_byte_record(&output_record)?;

                            Ok(())
                        })?;

                        *group = new_group;
                    }
                }
            };
        }

        if let Some(output_record) = program.run_with_record(index, &record)? {
            writer.write_byte_record(&output_record)?;
        }

        index += 1;
    }

    program.flush(index, |output_record| -> CliResult<()> {
        writer.write_byte_record(&output_record)?;

        Ok(())
    })?;

    Ok(writer.flush()?)
}
