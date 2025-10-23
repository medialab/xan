use std::cmp::Ordering;

use bstr::ByteSlice;
use rayon::slice::ParallelSliceMut;

use crate::config::{Config, Delimiter};
use crate::scales::LinearScale;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Discretize selection of columns containing continuous data into bins.

The bins table is formatted as CSV data:

    field,value,lower_bound,upper_bound,count

Usage:
    xan bins [options] [<input>]
    xan bins --help

bins options:
    -s, --select <arg>     Select a subset of columns to compute bins
                           for. See 'xan select --help' for the format
                           details.
    -b, --bins <number>    Number of bins. Will default to using various heuristics
                           to find an optimal default number if not provided.
    -E, --nice             Whether to choose nice boundaries for the bins.
                           Might return a number of bins slightly different to
                           what was passed to -b/--bins, as a consequence.
    -l, --label <mode>     Label to choose for the bins (that will be placed in the
                           `value` column). Mostly useful to tweak representation when
                           piping to `xan hist`. Can be one of \"full\", \"lower\" or \"upper\".
                           [default: full]
    -m, --min <min>        Override min value.
    -M, --max <max>        Override max value.
    -N, --no-extra         Don't include, nulls, nans and out-of-bounds counts.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_extra: bool,
    flag_bins: Option<usize>,
    flag_label: String,
    flag_nice: bool,
    flag_min: Option<f64>,
    flag_max: Option<f64>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    if !["full", "upper", "lower"].contains(&args.flag_label.as_str()) {
        Err(format!(
            "unknown --label {:?}, must be one of \"full\", \"upper\" or \"lower\".",
            args.flag_label
        ))?;
    }

    let mut rdr = conf.simd_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let headers = rdr.byte_headers()?.clone();
    let sel = conf.selection(&headers)?;

    let mut all_series: Vec<Series> = sel.iter().map(|i| Series::new(*i)).collect();

    let mut record = simd_csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        for (cell, series) in sel.select(&record).zip(all_series.iter_mut()) {
            series.add(cell, &args.flag_min, &args.flag_max);
        }
    }

    wtr.write_record(["field", "value", "lower_bound", "upper_bound", "count"])?;

    for series in all_series.iter_mut() {
        match series.bins(
            args.flag_bins,
            &args.flag_min,
            &args.flag_max,
            args.flag_nice,
        ) {
            None => continue,
            Some(bins) => {
                let max_lower_bound_width = bins
                    .iter()
                    .map(|bin| util::format_number(bin.lower_bound).len())
                    .max()
                    .unwrap();
                let max_upper_bound_width = bins
                    .iter()
                    .map(|bin| util::format_number(bin.upper_bound).len())
                    .max()
                    .unwrap();

                let mut bins_iter = bins.iter().peekable();

                while let Some(bin) = bins_iter.next() {
                    let (lower_bound, upper_bound) = match series.data_type {
                        DataType::Float => (bin.lower_bound, bin.upper_bound),
                        DataType::Integer => (bin.lower_bound.ceil(), bin.upper_bound.ceil()),
                    };

                    let lower_bound = util::format_number(lower_bound);
                    let upper_bound = util::format_number(upper_bound);

                    let label_format = if bin.is_constant() {
                        lower_bound
                    } else {
                        match args.flag_label.as_str() {
                            "full" => match bins_iter.peek() {
                                None => format!(
                                    ">= {:lower_width$} <= {:upper_width$}",
                                    lower_bound,
                                    upper_bound,
                                    lower_width = max_lower_bound_width,
                                    upper_width = max_upper_bound_width
                                ),
                                Some(_) => format!(
                                    ">= {:lower_width$} <  {:upper_width$}",
                                    lower_bound,
                                    upper_bound,
                                    lower_width = max_lower_bound_width,
                                    upper_width = max_upper_bound_width
                                ),
                            },
                            "upper" => upper_bound,
                            "lower" => lower_bound,
                            _ => unreachable!(),
                        }
                    };

                    wtr.write_record(vec![
                        &headers[series.column],
                        label_format.as_bytes(),
                        bin.lower_bound.to_string().as_bytes(),
                        bin.upper_bound.to_string().as_bytes(),
                        bin.count.to_string().as_bytes(),
                    ])?;
                }
            }
        }

        if !args.flag_no_extra && series.nans > 0 {
            wtr.write_record([
                &headers[series.column],
                b"<NaN>",
                b"",
                b"",
                series.nans.to_string().as_bytes(),
            ])?;
        }

        if !args.flag_no_extra && series.nulls > 0 {
            wtr.write_record([
                &headers[series.column],
                b"<null>",
                b"",
                b"",
                series.nulls.to_string().as_bytes(),
            ])?;
        }

        if !args.flag_no_extra && series.out_of_bounds > 0 {
            wtr.write_record([
                &headers[series.column],
                b"<rest>",
                b"",
                b"",
                series.out_of_bounds.to_string().as_bytes(),
            ])?;
        }
    }

    Ok(wtr.flush()?)
}

fn compute_rectified_iqr(numbers: &[f64], stats: &SeriesStats) -> Option<f64> {
    if numbers.len() < 4 {
        None
    } else {
        let q1 = (numbers.len() as f64 * 0.25).floor() as usize;
        let q3 = (numbers.len() as f64 * 0.75).floor() as usize;

        let mut q1 = numbers[q1];
        let mut q3 = numbers[q3];

        // Translating to avoid non-positive issues
        let offset = stats.min().unwrap() + 1.0;

        q1 += offset;
        q3 += offset;

        let iqr = q3 - q1;

        Some(iqr)
    }
}

#[derive(Debug)]
struct SeriesStats {
    extent: Option<(f64, f64)>,
}

impl SeriesStats {
    pub fn min(&self) -> Option<f64> {
        self.extent.map(|extent| extent.0)
    }

    pub fn max(&self) -> Option<f64> {
        self.extent.map(|extent| extent.1)
    }
}

#[derive(Debug)]
struct Bin {
    lower_bound: f64,
    upper_bound: f64,
    count: usize,
}

impl Bin {
    fn is_constant(&self) -> bool {
        self.lower_bound == self.upper_bound
    }
}

#[derive(Debug)]
enum DataType {
    Integer,
    Float,
}

#[derive(Debug)]
struct Series {
    column: usize,
    numbers: Vec<f64>,
    count: usize,
    nans: usize,
    nulls: usize,
    out_of_bounds: usize,
    data_type: DataType,
}

impl Series {
    pub fn new(column: usize) -> Self {
        Series {
            column,
            numbers: Vec::new(),
            count: 0,
            nans: 0,
            nulls: 0,
            out_of_bounds: 0,
            data_type: DataType::Integer,
        }
    }

    pub fn add(&mut self, cell: &[u8], min: &Option<f64>, max: &Option<f64>) {
        self.count += 1;

        let cell = cell.trim();

        if cell.is_empty() {
            self.nulls += 1;
            return;
        }

        match fast_float::parse::<f64, &[u8]>(cell) {
            Ok(float) => {
                if let Some(m) = min {
                    if float < *m {
                        self.out_of_bounds += 1;
                        return;
                    }
                } else if let Some(m) = max {
                    if float > *m {
                        self.out_of_bounds += 1;
                        return;
                    }
                }

                if float.fract() != 0.0 {
                    self.data_type = DataType::Float;
                }

                self.numbers.push(float);
            }
            Err(_) => {
                self.nans += 1;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.numbers.len()
    }

    pub fn stats(&self) -> SeriesStats {
        let mut extent: Option<(f64, f64)> = None;

        for n in self.numbers.iter() {
            let n = *n;

            extent = match extent {
                None => Some((n, n)),
                Some(m) => Some((f64::min(n, m.0), f64::max(n, m.1))),
            };
        }

        SeriesStats { extent }
    }

    pub fn naive_optimal_bin_count(&self) -> usize {
        usize::min((self.len() as f64).sqrt().ceil() as usize, 50)
    }

    pub fn freedman_diaconis(&mut self, width: f64, stats: &SeriesStats) -> Option<usize> {
        self.numbers.par_sort_unstable_by(|a, b| a.total_cmp(b));

        compute_rectified_iqr(&self.numbers, stats).and_then(|iqr| {
            if iqr == 0.0 {
                return None;
            }

            let bin_width = 2.0 * (iqr / (self.numbers.len() as f64).cbrt());

            Some((width / bin_width).ceil() as usize)
        })
    }

    pub fn optimal_bin_count(&mut self, width: f64, stats: &SeriesStats) -> usize {
        usize::max(
            2,
            self.freedman_diaconis(width, stats)
                .unwrap_or_else(|| self.naive_optimal_bin_count()),
        )
        .min(50)
    }

    pub fn bins(
        &mut self,
        count: Option<usize>,
        min: &Option<f64>,
        max: &Option<f64>,
        nice: bool,
    ) -> Option<Vec<Bin>> {
        if self.len() < 1 {
            return None;
        }

        let stats = self.stats();

        let min = min.unwrap_or_else(|| stats.min().unwrap());
        let max = max.unwrap_or_else(|| stats.max().unwrap());

        if min == max {
            return Some(vec![Bin {
                lower_bound: min,
                upper_bound: max,
                count: self.len(),
            }]);
        }

        let width = max - min;

        let count = count.unwrap_or_else(|| self.optimal_bin_count(width, &stats));

        let bins = if nice {
            let scale = LinearScale::nice((min, max), (0.0, 1.0), count);
            let mut ticks = scale.ticks(count);

            if ticks.is_empty() {
                return Some(vec![]);
            }

            if ticks.len() >= 2 {
                ticks[0] = min;
                *ticks.last_mut().unwrap() = max;
            }

            let mut bins: Vec<Bin> = Vec::with_capacity(ticks.len());

            for i in 0..(ticks.len() - 1) {
                bins.push(Bin {
                    lower_bound: ticks[i],
                    upper_bound: ticks[i + 1],
                    count: 0,
                });
            }

            for n in self.numbers.iter() {
                // NOTE: using `binary_search_by` as lower_bound
                let bin_index = bins
                    .binary_search_by(|bin| match bin.upper_bound.partial_cmp(n).unwrap() {
                        Ordering::Equal => Ordering::Less,
                        ord => ord,
                    })
                    .unwrap_err()
                    .min(bins.len().saturating_sub(1));

                bins[bin_index].count += 1;
            }

            bins
        } else {
            let mut bins: Vec<Bin> = Vec::with_capacity(count);

            let cell_width = width / count as f64;

            let mut lower_bound = min;

            for _ in 0..count {
                let upper_bound = f64::min(lower_bound + cell_width, max);

                bins.push(Bin {
                    lower_bound,
                    upper_bound,
                    count: 0,
                });

                lower_bound = upper_bound;
            }

            for n in self.numbers.iter() {
                let mut bin_index = ((n - min) / cell_width).floor() as usize;

                // Exception to include max in last bin
                if bin_index == bins.len() {
                    bin_index -= 1;
                }

                bins[bin_index].count += 1;
            }

            bins
        };

        Some(bins)
    }
}
