use std::num::NonZeroUsize;

use crate::cmd::parallel::Args as ParallelArgs;
use crate::collections::ClusteredInsertHashmap;
use crate::config::{Config, Delimiter};
use crate::moonblade::Stats;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

type GroupKey = Vec<Vec<u8>>;

static USAGE: &str = "
Computes descriptive statistics on CSV data.

By default, statistics are reported for *every* column in the CSV data. The default
set of statistics corresponds to statistics that can be computed efficiently on a
stream of data in constant memory, but more can be selected using flags documented
hereafter.

If you have more specific needs or want to perform custom aggregations, please be
sure to check the `xan agg` command instead.

Here is what the CSV output will look like:

field              (default) - Name of the described column
count              (default) - Number of non-empty values contained by the column
count_empty        (default) - Number of empty values contained by the column
type               (default) - Most likely type of the column
types              (default) - Pipe-separated list of all types witnessed in the column
sum                (default) - Sum of numerical values
mean               (default) - Mean of numerical values
q1                 (-q, -A)  - First quartile of numerical values
median             (-q, -A)  - Second quartile, i.e. median, of numerical values
q3                 (-q, -A)  - Third quartile of numerical values
variance           (default) - Population variance of numerical values
stddev             (default) - Population standard deviation of numerical values
min                (default) - Minimum numerical value
max                (default) - Maximum numerical value
approx_cardinality (-a)      - Approximation of the number of distinct string values
approx_q1          (-a)      - Approximation of the first quartile of numerical values
approx_median      (-a)      - Approximation of the median of numerical values
approx_q3          (-a)      - Approximation of the third quartile of numerical values
cardinality        (-c, -A)  - Number of distinct string values
mode               (-c, -A)  - Most frequent string value (tie breaking is arbitrary & random!)
tied_for_mode      (-c, -A)  - Number of values tied for mode
lex_first          (default) - First string in lexical order
lex_last           (default) - Last string in lexical order
min_length         (default) - Minimum string length
max_length         (default) - Maximum string length

Stats can be computed in parallel using the -p/--parallel or -t/--threads flags.
But note that this cannot work on streams or gzipped data and does not support
the -g/--groubpy flag.

Usage:
    xan stats [options] [<input>]

stats options:
    -s, --select <arg>       Select a subset of columns to compute stats for.
                             See 'xan select --help' for the format details.
                             This is provided here because piping 'xan select'
                             into 'xan stats' will disable the use of indexing.
    -g, --groupby <cols>     If given, will compute stats per group as defined by
                             the given column selection.
    -A, --all                Shorthand for -cq.
    -c, --cardinality        Show cardinality and modes.
                             This requires storing all CSV data in memory.
    -q, --quartiles          Show quartiles.
                             This requires storing all CSV data in memory.
    -a, --approx             Compute approximated statistics.
    --nulls                  Include empty values in the population size for computing
                             mean and standard deviation.
    -p, --parallel           Whether to use parallelization to speed up counting.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. i.e., They will be included
                           in statistics.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Clone, Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_groupby: Option<SelectColumns>,
    flag_all: bool,
    flag_cardinality: bool,
    flag_quartiles: bool,
    flag_approx: bool,
    flag_nulls: bool,
    flag_parallel: bool,
    flag_threads: Option<NonZeroUsize>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn new_stats(&self) -> Stats {
        let mut stats = Stats::new();

        if self.flag_nulls {
            stats.include_nulls();
        }

        if self.flag_all || self.flag_cardinality {
            stats.compute_frequencies();
        }

        if self.flag_all || self.flag_quartiles {
            stats.compute_numbers();
        }

        if self.flag_approx {
            stats.compute_approx();
        }

        stats
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_groupby.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -g/--groupby!")?;
        }

        let mut parallel_args = ParallelArgs::single_file(&args.arg_input, args.flag_threads)?;

        parallel_args.cmd_stats = true;
        parallel_args.flag_select = args.flag_select;
        parallel_args.flag_all = args.flag_all;
        parallel_args.flag_cardinality = args.flag_cardinality;
        parallel_args.flag_quartiles = args.flag_quartiles;
        parallel_args.flag_approx = args.flag_approx;
        parallel_args.flag_nulls = args.flag_nulls;

        return parallel_args.run();
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select.clone());

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let mut sel = rconf.selection(&headers)?;
    let groupby_sel_opt = args
        .flag_groupby
        .as_ref()
        .map(|cols| cols.selection(&headers, !args.flag_no_headers))
        .transpose()?;

    // No need to consider the grouping column when aggregating stats
    if let Some(gsel) = &groupby_sel_opt {
        sel.subtract(gsel);
    }

    // Nothing was selected
    if sel.is_empty() {
        return Ok(());
    }

    let field_names: Vec<Vec<u8>> = if args.flag_no_headers {
        sel.iter()
            .map(|i| i.to_string().as_bytes().to_vec())
            .collect()
    } else {
        sel.select(&headers).map(|h| h.to_vec()).collect()
    };

    // Grouping
    if let Some(gsel) = groupby_sel_opt {
        let mut record = csv::ByteRecord::new();

        for h in gsel.select(&headers) {
            record.push_field(h);
        }

        record.extend(&args.new_stats().headers());

        wtr.write_byte_record(&record)?;

        let mut groups: ClusteredInsertHashmap<GroupKey, Vec<Stats>> =
            ClusteredInsertHashmap::new();

        while rdr.read_byte_record(&mut record)? {
            let group_key: Vec<_> = gsel.select(&record).map(|cell| cell.to_vec()).collect();

            groups.insert_with_or_else(
                group_key,
                || {
                    let mut fields = (0..sel.len()).map(|_| args.new_stats()).collect::<Vec<_>>();

                    for (cell, stats) in sel.select(&record).zip(fields.iter_mut()) {
                        stats.process(cell);
                    }

                    fields
                },
                |fields| {
                    for (cell, stats) in sel.select(&record).zip(fields.iter_mut()) {
                        stats.process(cell);
                    }
                },
            );
        }

        for (group, fields) in groups.into_iter() {
            for (name, stats) in field_names.iter().zip(fields.into_iter()) {
                record.clear();

                for h in group.iter() {
                    record.push_field(h);
                }

                record.extend(&stats.results(name));

                wtr.write_byte_record(&record)?;
            }
        }

        return Ok(wtr.flush()?);
    }

    // No grouping
    let mut fields = (0..sel.len()).map(|_| args.new_stats()).collect::<Vec<_>>();

    wtr.write_byte_record(&fields[0].headers())?;

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        for (cell, stats) in sel.select(&record).zip(fields.iter_mut()) {
            stats.process(cell);
        }
    }

    for (name, stats) in field_names.into_iter().zip(fields.into_iter()) {
        wtr.write_byte_record(&stats.results(&name))?;
    }

    Ok(wtr.flush()?)
}
