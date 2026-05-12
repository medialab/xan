use std::cmp::Ordering;
use std::io::{stdout, Write};
use std::num::NonZeroUsize;

use bstr::ByteSlice;
use colored::Colorize;
use pad::{Alignment, PadStr};
use simd_csv::ByteRecord;
use unicode_width::UnicodeWidthStr;

use crate::cmd::parallel::Args as ParallelArgs;
use crate::cmd::spark::SparklineRendererOptions;
use crate::collections::{ClusteredInsertHashmap, Counter};
use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicNumber, Stats, Welford};
use crate::scales::{Extent, ExtentBuilder, Histogram, Scale, ScaleType};
use crate::select::SelectedColumns;
use crate::util::{self, format_number, ColorMode, ColorOrStyles, FALSE_VALUES, TRUE_VALUES};
use crate::CliResult;

const LABELS_TO_SHOW: usize = 5;
const CATEGORIES_TO_SHOW: usize = 10;
const HISTOGRAM_BINS: usize = 35;
const CARDINALITY_RATIO_THRESHOLD: f64 = 0.7;

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
        is_int: bool,
        extent: Extent<f64>,
        mean: f64,
        median: f64,
        stddev: f64,
        histogram: Histogram,
    },
    Categorical {
        is_bool: bool,
        cardinality: u64,
        top: Vec<(String, u64)>,
        total: u64,
    },
    Labels {
        sample: Vec<String>,
        length_extent: Extent<f64>,
        length_histogram: Histogram,
    },
    Void,
}

impl ColumnType {
    fn as_str(&self) -> &str {
        match self {
            Self::Numerical { is_int, .. } => {
                if *is_int {
                    "numerical (integers)"
                } else {
                    "numerical (floats)"
                }
            }
            Self::Categorical { is_bool, .. } => {
                if *is_bool {
                    "boolean"
                } else {
                    "categorical"
                }
            }
            Self::Labels { length_extent, .. } => {
                if length_extent.max() > 250.0 {
                    "text"
                } else {
                    "labels"
                }
            }
            Self::Void => "void",
        }
    }
}

#[derive(Debug)]
struct ColumnEstimator {
    name: String,
    strings: Counter<Vec<u8>>,
    numbers: Vec<f64>,
    numerical_welford: Welford,
    numerical_extent_builder: ExtentBuilder<f64>,
    length_extent_builder: ExtentBuilder<f64>,
    int_count: u64,
    empty_count: u64,
    count: u64,
    first_seen: Vec<Vec<u8>>,
    // first_seen_int: (i64, u64), conflate with int_count later and spillover for bool detection
}

impl ColumnEstimator {
    fn new(name: &[u8]) -> Self {
        Self {
            name: String::from_utf8_lossy(name).into_owned(),
            strings: Counter::new(None),
            numbers: Vec::new(),
            numerical_welford: Welford::new(),
            numerical_extent_builder: ExtentBuilder::new(),
            length_extent_builder: ExtentBuilder::new(),
            int_count: 0,
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

            self.numerical_welford.add(f);
            self.numerical_extent_builder.process(f);
            self.numbers.push(f);
        } else {
            self.length_extent_builder.process(cell.len() as f64);
            self.strings.add(cell.to_vec());
        }
    }

    fn name_hash(&self) -> usize {
        let mut sum: usize = 0;

        for byte in self.name.as_bytes() {
            sum += *byte as usize;
        }

        sum
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
        !self.is_void() && self.non_empty_count() == self.numerical_welford.count() as u64
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
            let extent = self.numerical_extent_builder.build().unwrap();

            let histogram =
                Histogram::from_extent_and_series(HISTOGRAM_BINS, extent, &self.numbers);

            ColumnType::Numerical {
                is_int: self.is_int(),
                extent,
                mean: self.numerical_welford.mean().unwrap(),
                median: linear_time_median(&mut self.numbers),
                stddev: self.numerical_welford.stdev().unwrap(),
                histogram,
            }
        } else if self.string_cardinality_ratio() < CARDINALITY_RATIO_THRESHOLD {
            let (total, top) = self
                .strings
                .clone()
                .into_total_and_items(Some(CATEGORIES_TO_SHOW), false);

            let mut is_bool = false;

            if top.len() == 2
                && (TRUE_VALUES.contains(&top[0].0.as_ref())
                    && FALSE_VALUES.contains(&top[1].0.as_ref()))
                || (TRUE_VALUES.contains(&top[1].0.as_ref())
                    && FALSE_VALUES.contains(&top[0].0.as_ref()))
            {
                is_bool = true;
            }

            ColumnType::Categorical {
                is_bool,
                cardinality: self.string_cardinality(),
                top: top
                    .iter()
                    .map(|(cell, count)| (String::from_utf8_lossy(cell).into_owned(), *count))
                    .collect(),
                total,
            }
        } else if self.is_void() {
            ColumnType::Void
        } else {
            let length_extent = self.length_extent_builder.build().unwrap();
            let mut length_histogram = Histogram::new(HISTOGRAM_BINS, length_extent);
            let mut length_welford = Welford::new();

            for (name, count) in self.strings.iter() {
                let length = name.len() as f64;

                length_welford.add_n(length, count as usize);
                length_histogram.add_n(length, count as usize);
            }

            ColumnType::Labels {
                sample: self.to_sample(),
                length_extent,
                length_histogram,
            }
        }
    }
}

static USAGE: &str = "
Computes descriptive statistics of CSV data.

If you want to print human-readable output, use the -D/--describe flag.

Else this command can be used to generate a CSV output that can be easily piped
into other `xan` commands.

By default, statistics are reported for *every* column in the CSV data, but you
can restrict the set of analyzed columns using the -s/--select flag.

The default set of statistics corresponds to things that can be computed efficiently
on a stream in constant memory, but more can be selected using flags documented
hereafter.

Stats can also be computed per group using the -g/--groupby flag.

If you have more specific needs or want to perform custom aggregations, please be
sure to check the `xan agg` or `xan groupby` commands instead.

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

Note that the output of the -D/--describe can easily be piped into a pager
thusly (don't forget to force colors):

    $ xan stats -D data.csv --color always | less -SR

Usage:
    xan stats [options] [<input>]

stats options:
    -s, --select <arg>       Select a subset of columns to compute stats for.
                             See 'xan select --help' for the format details.
                             This is provided here because piping 'xan select'
                             into 'xan stats' will disable the use of indexing.
    -D, --describe           Produce a human-readable output suitable to understand
                             what your columns contain, along with the relevant
                             dataviz (bar charts, top lists etc.)
                             Does not work with -g/--groupby.
    --sep <str>              Indicate that cells must be split using given separator.
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

stats -D/--describe options:
    --cols <num>    Width of the graph in terminal columns, i.e. characters.
                    Defaults to using all your terminal's width or 80 if
                    terminal's size cannot be found (i.e. when piping to file).
                    Can also be given as a ratio or percentage of the terminal's width
                    e.g. \"45%\" or \"0.5\".
    --color <when>  When to color the output using ANSI escape codes.
                    Use `auto` for automatic detection, `never` to
                    disable colors completely and `always` to force
                    colors, even when the output could not handle them.
                    [default: auto]

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
    flag_color: ColorMode,
    flag_cols: Option<String>,
    flag_sep: Option<String>,
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
        args.flag_color.apply();

        if args.flag_groupby.is_some() {
            Err("-D/--describe does not work with -g/--groupby!")?;
        }

        let mut out = stdout();
        let cols = util::acquire_term_cols_ratio(&args.flag_cols)?;

        let mut estimators: Vec<_> = sel
            .select(&headers)
            .enumerate()
            .map(|(i, name)| {
                if rconf.no_headers {
                    ColumnEstimator::new(format!("Column n°{}", i).as_bytes())
                } else {
                    ColumnEstimator::new(name)
                }
            })
            .collect();

        while rdr.read_byte_record(&mut record)? {
            for (estimator, cell) in estimators.iter_mut().zip(sel.select(&record)) {
                if let Some(sep) = &args.flag_sep {
                    for sub_cell in cell.split_str(sep) {
                        estimator.process(sub_cell);
                    }
                } else {
                    estimator.process(cell);
                }
            }
        }

        let sep = "─".repeat(cols).dimmed();

        for mut estimator in estimators {
            let column_type = estimator.infer_type();

            writeln!(
                &mut out,
                "{}",
                estimator
                    .name
                    .pad_to_width_with_alignment(cols, Alignment::Left)
                    .on_cyan()
                    .bold()
            )?;

            if args.flag_color.is_never() {
                writeln!(&mut out, "{}", sep)?;
            }

            writeln!(&mut out, "{}", column_type.as_str())?;
            writeln!(&mut out, "cells: {}", format_number(estimator.count).red())?;

            if estimator.empty_count != 0 {
                let ratio = estimator.empty_count as f64 / estimator.count as f64;

                writeln!(
                    &mut out,
                    "empty cells: {} ({:.2}%)",
                    format_number(estimator.empty_count).dimmed(),
                    ratio * 100.0
                )?;
            }

            match column_type {
                ColumnType::Void => {
                    writeln!(&mut out, "\nThere is nothing is this column but the endless depths of the void!\nZilch, nada, rien, keud!")?;
                }
                ColumnType::Categorical {
                    is_bool,
                    cardinality,
                    top,
                    total,
                } => {
                    let mut remaining = total;
                    let name_hash = estimator.name_hash();

                    for (_, count) in top.iter() {
                        remaining -= *count;
                    }

                    if !is_bool {
                        writeln!(
                            &mut out,
                            "distinct values: {}",
                            format_number(cardinality).red()
                        )?;
                        writeln!(&mut out, "top {} values:", top.len().to_string().red())?;
                    }

                    let max_count_width = top
                        .iter()
                        .map(|(_, count)| format_number(*count).len())
                        .max()
                        .unwrap()
                        .max(format_number(remaining).len());

                    let max_value_cols = cols.saturating_sub(max_count_width).saturating_sub(16);
                    let mut histogram = vec![0.0; top.len() + (remaining > 0) as usize];
                    let mut color_overrides = vec![None; histogram.len()];

                    for (i, (value, count)) in top.iter().enumerate() {
                        let color = util::colorizer_by_rainbow_with_fallback(i, name_hash, value);
                        histogram[i] = *count as f64;
                        color_overrides[i] = Some(color);

                        writeln!(
                            &mut out,
                            " {}  {:>2} {:>width$} {} {}",
                            color.colorize("■"),
                            (i + 1).to_string().dimmed(),
                            format_number(*count).cyan(),
                            format!("{:>6.2}%", (*count as f64 / total as f64) * 100.0).magenta(),
                            util::highlight_problematic_string_features(
                                &util::unicode_aware_ellipsis(
                                    &util::sanitize_text_for_single_line_printing(value),
                                    max_value_cols
                                )
                            )
                            .green(),
                            width = max_count_width
                        )?;
                    }

                    if remaining > 0 {
                        *histogram.last_mut().unwrap() = remaining as f64;
                        *color_overrides.last_mut().unwrap() = Some(ColorOrStyles::dimmed());

                        writeln!(
                            &mut out,
                            " {}     {:>width$} {} {}",
                            "■".dimmed(),
                            format_number(remaining).cyan(),
                            format!("{:>6.2}%", (remaining as f64 / total as f64) * 100.0)
                                .magenta(),
                            util::unicode_aware_ellipsis("<rest>", max_value_cols).dimmed(),
                            width = max_count_width
                        )?;
                    }

                    let sparkline_scale =
                        Scale::new(ScaleType::Linear, (0.0, top[0].1 as f64), (0.0, 1.0));

                    let mut sparkline_renderer_options = SparklineRendererOptions::new();
                    sparkline_renderer_options.height = 5;
                    sparkline_renderer_options.width = 3;

                    let mut sparkline_renderer = sparkline_renderer_options.build();
                    sparkline_renderer.render_with_color_overrides(
                        &sparkline_scale,
                        &histogram,
                        Some(&color_overrides),
                    );

                    writeln!(&mut out, "\n{}", sparkline_renderer)?;

                    for i in 0..top.len() {
                        write!(&mut out, "{:^3}", (i + 1).to_string().dimmed())?;
                    }

                    if remaining > 0 {
                        write!(&mut out, "{:^3}", "r".dimmed())?;
                    }

                    writeln!(&mut out)?;
                }
                ColumnType::Numerical {
                    extent,
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
                        format_number(extent.min()).blue(),
                        format_number(extent.max()).red()
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

                    let scale_denomination = if histogram.should_use_log_scale() {
                        histogram.ln_1p();

                        "log"
                    } else {
                        "linear"
                    };

                    let sparkline_scale =
                        Scale::new(ScaleType::Linear, (0.0, histogram.max_value()), (0.0, 1.0));

                    let mut sparkline_renderer_options = SparklineRendererOptions::new();
                    sparkline_renderer_options.height = 5;
                    sparkline_renderer_options.set_striped();

                    let mut sparkline_renderer = sparkline_renderer_options.build();
                    sparkline_renderer.render(&sparkline_scale, &histogram);
                    sparkline_renderer.render_central_tendency(
                        histogram.bins(),
                        histogram.discrete_index(mean),
                        histogram.discrete_index(median),
                        histogram.discrete_index(mean - stddev),
                        histogram.discrete_index(mean + stddev),
                    );

                    writeln!(
                        &mut out,
                        "distribution ({} scale, {}/total): {}/{} ({:.2}%)\n{}",
                        scale_denomination.cyan(),
                        "highest".red(),
                        format_number(histogram.max_count()).red(),
                        format_number(estimator.count),
                        (histogram.max_count() as f64 / estimator.count as f64) * 100.0,
                        sparkline_renderer
                    )?;
                }
                ColumnType::Labels {
                    sample,
                    mut length_histogram,
                    length_extent,
                } => {
                    writeln!(
                        &mut out,
                        "First {} non empty values{}:",
                        sample.len(),
                        if sample.iter().any(|value| value.width() >= 500) {
                            " (truncated)"
                        } else {
                            ""
                        }
                    )?;

                    for value in sample {
                        writeln!(
                            &mut out,
                            "  - {}",
                            util::wrap(
                                &util::unicode_aware_ellipsis(&value, 500),
                                cols.saturating_sub(4),
                                4
                            )
                            .green()
                        )?;
                    }

                    if length_extent.is_constant() {
                        writeln!(
                            &mut out,
                            "all have same length: {}",
                            length_extent.min().to_string().red()
                        )?;
                    } else {
                        let scale_denomination = if length_histogram.should_use_log_scale() {
                            length_histogram.ln_1p();

                            "log"
                        } else {
                            "linear"
                        };

                        let sparkline_scale = Scale::new(
                            ScaleType::Linear,
                            (0.0, length_histogram.max_value()),
                            (0.0, 1.0),
                        );

                        let mut sparkline_renderer_options = SparklineRendererOptions::new();
                        sparkline_renderer_options.height = 3;
                        sparkline_renderer_options.set_striped();

                        let mut sparkline_renderer = sparkline_renderer_options.build();
                        sparkline_renderer.render(&sparkline_scale, &length_histogram);

                        writeln!(
                            &mut out,
                            "length distribution ({} scale, {} … {}): {} … {}",
                            scale_denomination.cyan(),
                            "min".blue(),
                            "max".red(),
                            length_extent.min().to_string().blue(),
                            length_extent.max().to_string().red()
                        )?;
                        writeln!(&mut out, "{}", sparkline_renderer)?;
                    }
                }
            };

            writeln!(&mut out, "\n")?;
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
                if let Some(sep) = &args.flag_sep {
                    for sub_cell in cell.split_str(sep) {
                        stats.process(sub_cell);
                    }
                } else {
                    stats.process(cell);
                }
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
            if let Some(sep) = &args.flag_sep {
                for sub_cell in cell.split_str(sep) {
                    stats.process(sub_cell);
                }
            } else {
                stats.process(cell);
            }
        }
    }

    for (name, stats) in field_names.into_iter().zip(fields.into_iter()) {
        wtr.write_byte_record(&stats.results(&name))?;
    }

    Ok(wtr.flush()?)
}
