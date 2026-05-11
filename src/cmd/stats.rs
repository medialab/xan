use std::cmp::Ordering;
use std::io::{stdout, Write};
use std::num::NonZeroUsize;

use bstr::BString;
use colored::Colorize;
use simd_csv::ByteRecord;

use crate::cmd::parallel::Args as ParallelArgs;
use crate::cmd::spark::SparklineRendererOptions;
use crate::collections::{ClusteredInsertHashmap, Counter};
use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicNumber, Stats, Welford};
use crate::scales::{ExtentBuilder, Scale, ScaleType};
use crate::select::SelectedColumns;
use crate::util::{self, format_number};
use crate::CliResult;

const LABELS_TO_SHOW: usize = 5;
// const CATEGORIES_TO_SHOW: usize = 7;
const HISTOGRAM_BINS: usize = 35;

fn float_cmp(a: &f64, b: &f64) -> Ordering {
    a.partial_cmp(b).unwrap()
}

fn linear_time_median(values: &mut [f64]) -> f64 {
    let n = values.len();
    let mid = n / 2;

    values.select_nth_unstable_by(mid, float_cmp);

    if n % 2 == 1 {
        values[mid]
    } else {
        let lower = *values[..mid].iter().max_by(|a, b| float_cmp(a, b)).unwrap();
        (lower + values[mid]) / 2.0
    }
}

enum ColumnType {
    Numerical {
        int: bool,
        min: f64,
        max: f64,
        mean: f64,
        median: f64,
        stddev: f64,
        histogram: Vec<f64>,
    },
    Categorical {
        cardinality: u64,
    },
    Labels {
        sample: Vec<String>,
    },
    Void,
}

impl ColumnType {
    fn as_str(&self) -> &str {
        match self {
            Self::Numerical { int, .. } => {
                if *int {
                    "numerical (integers)"
                } else {
                    "numerical (floats)"
                }
            }
            Self::Categorical { .. } => "categorical",
            Self::Labels { .. } => "labels",
            Self::Void => "void",
        }
    }
}

// TODO: untrimmed values counting
// TODO: example values, viz etc.
// TODO: label
// TODO: auto log scale
// TODO: timestamp
// TODO: --color
// TODO: print total count, empty count (with empty)

#[derive(Debug)]
struct ColumnEstimator {
    name: BString,
    strings: Counter<Vec<u8>>,
    numbers: Vec<f64>,
    welford: Welford,
    extent_builder: ExtentBuilder<f64>,
    int_count: u64,
    string_count: u64,
    empty_count: u64,
    count: u64,
    first_seen: Vec<Vec<u8>>,
}

impl ColumnEstimator {
    fn new(name: &[u8]) -> Self {
        Self {
            name: BString::from(name),
            strings: Counter::new(None),
            numbers: Vec::new(),
            welford: Welford::new(),
            extent_builder: ExtentBuilder::new(),
            int_count: 0,
            string_count: 0,
            empty_count: 0,
            count: 0,
            first_seen: Vec::with_capacity(LABELS_TO_SHOW),
        }
    }

    fn process(&mut self, cell: &[u8]) {
        self.count += 1;

        if cell.is_empty() {
            self.empty_count += 1;
            return;
        } else if self.first_seen.len() < LABELS_TO_SHOW {
            self.first_seen.push(cell.to_vec());
        }

        if let Ok(n) = DynamicNumber::try_from(cell) {
            if !n.is_float() {
                self.int_count += 1;
            }

            let f = n.as_float();

            self.welford.add(f);
            self.extent_builder.process(f);
            self.numbers.push(f);
        } else {
            self.string_count += 1;
            self.strings.add(cell.to_vec());
        }
    }

    fn non_empty_count(&self) -> u64 {
        self.count - self.empty_count
    }

    fn string_cardinality(&self) -> u64 {
        self.strings.cardinality()
    }

    fn string_cardinality_ratio(&self) -> f64 {
        self.string_cardinality() as f64 / self.non_empty_count() as f64
    }

    fn is_void(&self) -> bool {
        self.empty_count == self.count
    }

    fn is_numerical(&self) -> bool {
        !self.is_void() && self.non_empty_count() == self.welford.count() as u64
    }

    fn is_int(&self) -> bool {
        self.non_empty_count() == self.int_count
    }

    fn to_sample(&self) -> Vec<String> {
        self.first_seen
            .iter()
            .map(|cell| String::from_utf8_lossy(cell).into_owned())
            .collect()
    }

    fn infer_type(&mut self) -> ColumnType {
        if self.is_numerical() {
            let extent = self.extent_builder.build().unwrap();

            let bins = HISTOGRAM_BINS;
            let mut histogram = vec![0f64; bins];
            let cell_width = extent.width() / bins as f64;

            for x in self.numbers.iter().copied() {
                let index = (((x - extent.min()) / cell_width).floor() as usize).min(bins - 1);
                histogram[index] += 1.0;
            }

            ColumnType::Numerical {
                int: self.is_int(),
                min: extent.min(),
                max: extent.max(),
                mean: self.welford.mean().unwrap(),
                median: linear_time_median(&mut self.numbers),
                stddev: self.welford.stdev().unwrap(),
                histogram,
            }
        } else if self.string_cardinality_ratio() < 0.7 {
            ColumnType::Categorical {
                cardinality: self.string_cardinality(),
            }
        } else if self.is_void() {
            ColumnType::Void
        } else {
            ColumnType::Labels {
                sample: self.to_sample(),
            }
        }
    }
}

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
log_dist           (-q, -A)  - Sparkline (e.g. ▇▅▄▃▂▃▂▂▂▂) representing numerical distribution
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
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -g/--groupby option.

Usage:
    xan stats [options] [<input>]

stats options:
    -s, --select <arg>       Select a subset of columns to compute stats for.
                             See 'xan select --help' for the format details.
                             This is provided here because piping 'xan select'
                             into 'xan stats' will disable the use of indexing.
    -D, --describe
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
    -p, --parallel           Whether to use parallelization to speed up computation.
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
    flag_select: SelectedColumns,
    flag_groupby: Option<SelectedColumns>,
    flag_describe: bool,
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

        parallel_args.flag_no_headers = args.flag_no_headers;
        parallel_args.flag_output = args.flag_output;
        parallel_args.flag_delimiter = args.flag_delimiter;

        return parallel_args.run();
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select.clone());

    let mut rdr = rconf.simd_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let headers = rdr.byte_headers()?.clone();
    let mut sel = rconf.selection(&headers)?;
    let groupby_sel_opt = args
        .flag_groupby
        .as_ref()
        .map(|cols| cols.selection(&headers, !rconf.no_headers))
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

    let mut record = ByteRecord::new();

    // Describe
    if args.flag_describe {
        if args.flag_groupby.is_some() {
            Err("-D/--describe does not work with -g/--groupby!")?;
        }

        let mut out = stdout();
        let cols = util::acquire_term_cols(&None);

        let mut estimators: Vec<_> = sel.select(&headers).map(ColumnEstimator::new).collect();

        while rdr.read_byte_record(&mut record)? {
            for (estimator, cell) in estimators.iter_mut().zip(sel.select(&record)) {
                estimator.process(cell);
            }
        }

        let sep = "─".repeat(cols).dimmed();

        for mut estimator in estimators {
            let column_type = estimator.infer_type();

            writeln!(&mut out, "{}", sep)?;
            writeln!(
                &mut out,
                "{}: {}",
                String::from_utf8_lossy(&estimator.name).cyan(),
                column_type.as_str()
            )?;
            writeln!(&mut out, "{}", sep)?;

            writeln!(&mut out, "rows: {}", estimator.count.to_string().red())?;

            if estimator.empty_count != 0 {
                let ratio = estimator.empty_count as f64 / estimator.count as f64;

                writeln!(
                    &mut out,
                    "empty cells: {} ({:.2}%)",
                    estimator.empty_count.to_string().dimmed(),
                    ratio * 100.0
                )?;
            }

            match column_type {
                ColumnType::Void => {
                    writeln!(&mut out, "\nThere is nothing is this column but the endless depths of the void!\nZilch, nada, rien, keud!")?;
                }
                ColumnType::Categorical { cardinality } => {
                    writeln!(
                        &mut out,
                        "distinct values: {}",
                        format_number(cardinality).red()
                    )?;
                }
                ColumnType::Numerical {
                    min,
                    max,
                    mean,
                    median,
                    stddev,
                    mut histogram,
                    ..
                } => {
                    writeln!(
                        &mut out,
                        "({} … {}): {} … {}",
                        "min".blue(),
                        "max".red(),
                        format_number(min).blue(),
                        format_number(max).red()
                    )?;
                    writeln!(
                        &mut out,
                        "({} ± {}, {}): {} ± {}, {}",
                        "mean".green(),
                        "σ".magenta(),
                        "median".yellow(),
                        format_number(mean).green(),
                        format_number(stddev).magenta(),
                        format_number(median).yellow(),
                    )?;

                    // TODO: factorize into Histogram struct since we are going
                    // to use this later on...
                    let histogram_max = *histogram.iter().max_by(|a, b| float_cmp(a, b)).unwrap();

                    let mut histogram_max_for_scale = histogram_max;

                    let scale_denomination = if max / min > 1_000.0 {
                        for bin in histogram.iter_mut() {
                            *bin = bin.ln_1p();
                        }

                        histogram_max_for_scale = histogram_max.ln_1p();

                        "log"
                    } else {
                        "linear"
                    };

                    let sparkline_scale = Scale::new(
                        ScaleType::Linear,
                        (0.0, histogram_max_for_scale),
                        (0.0, 1.0),
                    );

                    let mut sparkline_renderer_options = SparklineRendererOptions::new();
                    sparkline_renderer_options.height = 5;
                    sparkline_renderer_options.set_striped();

                    let mut sparkline_renderer = sparkline_renderer_options.build();
                    sparkline_renderer.render(&sparkline_scale, &histogram);

                    writeln!(
                        &mut out,
                        "distribution ({} scale, {}/total): {}/{}\n{}",
                        scale_denomination.cyan(),
                        "highest".red(),
                        format_number(histogram_max).red(),
                        format_number(estimator.count),
                        sparkline_renderer
                    )?;
                }
                ColumnType::Labels { sample } => {
                    writeln!(&mut out, "First {} non empty values:", sample.len())?;

                    for value in sample {
                        writeln!(
                            &mut out,
                            "  - {}",
                            util::wrap(&value, cols.saturating_sub(4), 4).green()
                        )?;
                    }
                }
            };

            writeln!(&mut out, "{}\n", sep)?;
        }

        return Ok(());
    }

    // Grouping
    if let Some(gsel) = groupby_sel_opt {
        for h in gsel.select(&headers) {
            record.push_field(h);
        }

        record.extend(&args.new_stats().headers());

        wtr.write_byte_record(&record)?;

        let mut groups: ClusteredInsertHashmap<ByteRecord, Vec<Stats>> =
            ClusteredInsertHashmap::new();

        while rdr.read_byte_record(&mut record)? {
            let group_key = gsel.select(&record).collect();

            let fields = groups.insert_with(group_key, || {
                (0..sel.len()).map(|_| args.new_stats()).collect::<Vec<_>>()
            });

            for (cell, stats) in sel.select(&record).zip(fields.iter_mut()) {
                stats.process(cell);
            }
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
