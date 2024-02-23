use config::{Config, Delimiter};
use csv;
use select::SelectColumns;
use util;
use CliResult;

use moonblade::Stats;

static USAGE: &str = "
Computes basic statistics on CSV data.

Basic statistics includes mean, median, mode, standard deviation, sum, max and
min values. Note that some statistics are expensive to compute, so they must
be enabled explicitly. By default, statistics are reported for *every* column
in the CSV data. The default set of statistics corresponds to statistics that
can be computed efficiently on a stream of data in constant memory.

If you need very precise statistics and/or custom aggregation, please be sure
to check the `xan agg` command instead.

Usage:
    xan stats [options] [<input>]

stats options:
    -s, --select <arg>     Select a subset of columns to compute stats for.
                           See 'xan select --help' for the format details.
                           This is provided here because piping 'xan select'
                           into 'xan stats' will disable the use of indexing.
    -A, --all              Show all statistics available.
    --cardinality          Show cardinality and modes.
                           This requires storing all CSV data in memory.
    --quantiles            Show quantiles.
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
                           Must be a single character. [default: ,]
";

#[derive(Clone, Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_all: bool,
    flag_cardinality: bool,
    flag_quantiles: bool,
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

        if self.flag_all || self.flag_quantiles {
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
