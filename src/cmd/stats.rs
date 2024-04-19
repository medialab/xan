use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;
use csv;

use crate::moonblade::Stats;

static USAGE: &str = "
Computes descriptive statistics on CSV data.

By default, statistics are reported for *every* column in the CSV data. The default
set of statistics corresponds to statistics that can be computed efficiently on a
stream of data in constant memory, but more can be selected using flags documented
hereafter.

If you have more specific needs or want to perform custom aggregations, please be
sure to check the `xan agg` command instead.

Here is what the CSV output will look like:

field         (default) - Name of the described column
count         (default) - Number of non-empty values contained by the column
count_empty   (default) - Number of empty values contained by the column
type          (default) - Most likely type of the column
types         (default) - Pipe-separated list of all types witnessed in the column
sum           (default) - Sum of numerical values
mean          (default) - Mean of numerical values
q1            (-q)      - First quartile of numerical values
median        (-q)      - Second quartile, i.e. median, of numerical values
q3            (-q)      - Third quartile of numerical values
variance      (default) - Population variance of numerical values
stddev        (default) - Population standard deviation of numerical values
min           (default) - Minimum numerical value
max           (default) - Maximum numerical value
cardinality   (-c)      - Number of distinct string values
mode          (-c)      - Most frequent string value (tie breaking is arbitrary & random!)
tied_for_mode (-c)      - Number of values tied for mode
lex_first     (default) - First string in lexical order
lex_last      (default) - Last string in lexical order
min_length    (default) - Minimum string length
max_length    (default) - Maximum string length

Usage:
    xan stats [options] [<input>]

stats options:
    -s, --select <arg>     Select a subset of columns to compute stats for.
                           See 'xan select --help' for the format details.
                           This is provided here because piping 'xan select'
                           into 'xan stats' will disable the use of indexing.
    -A, --all              Show all statistics available.
    -c, --cardinality      Show cardinality and modes.
                           This requires storing all CSV data in memory.
    -q, --quartiles        Show quartiles.
                           This requires storing all CSV data in memory.
    --nulls                Include empty values in the population size for computing
                           mean and standard deviation.

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
    flag_all: bool,
    flag_cardinality: bool,
    flag_quartiles: bool,
    flag_nulls: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn new_stats_for_column(&self) -> Stats {
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

        stats
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select.clone());

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let sel = rconf.selection(&headers)?;

    // Nothing was selected
    if sel.len() == 0 {
        return Ok(());
    }

    let mut fields = (0..sel.len())
        .map(|_| args.new_stats_for_column())
        .collect::<Vec<_>>();

    wtr.write_byte_record(&fields[0].headers())?;

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        for (cell, stats) in sel.select(&record).zip(fields.iter_mut()) {
            stats.process(cell);
        }
    }

    let field_names: Vec<Vec<u8>> = if args.flag_no_headers {
        sel.indices()
            .map(|i| i.to_string().as_bytes().to_vec())
            .collect()
    } else {
        sel.select(&headers).map(|h| h.to_vec()).collect()
    };

    for (name, stats) in field_names.into_iter().zip(fields.into_iter()) {
        wtr.write_byte_record(&stats.results(&name))?;
    }

    Ok(wtr.flush()?)
}
