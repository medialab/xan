use simd_csv::ByteRecord;

use crate::CliResult;
use crate::collections::{HashMap, IndexSet, new_index_set};
use crate::config::{Config, Delimiter};
use crate::moonblade::agg::CovarianceWelford;
use crate::select::SelectedColumns;
use crate::util;

enum Axes {
    Homogeneous(IndexSet<Vec<u8>>),
    Heterogeneous {
        x: IndexSet<Vec<u8>>,
        y: IndexSet<Vec<u8>>,
    },
}

impl Axes {
    fn new(heterogeneous: bool) -> Self {
        if heterogeneous {
            Self::Heterogeneous {
                x: new_index_set(),
                y: new_index_set(),
            }
        } else {
            Self::Homogeneous(new_index_set())
        }
    }

    fn insert_x(&mut self, label: Vec<u8>) -> usize {
        match self {
            Self::Homogeneous(labels) => labels.insert_full(label).0,
            Self::Heterogeneous { x, .. } => x.insert_full(label).0,
        }
    }

    fn insert_y(&mut self, label: Vec<u8>) -> usize {
        match self {
            Self::Homogeneous(labels) => labels.insert_full(label).0,
            Self::Heterogeneous { y, .. } => y.insert_full(label).0,
        }
    }

    fn shape(&self) -> (usize, usize) {
        match self {
            Self::Homogeneous(labels) => (labels.len(), labels.len()),
            Self::Heterogeneous { x, y } => (x.len(), y.len()),
        }
    }

    fn x_labels(&self) -> impl Iterator<Item = &Vec<u8>> {
        match self {
            Self::Homogeneous(labels) => labels.iter(),
            Self::Heterogeneous { x, .. } => x.iter(),
        }
    }

    fn get_y_label(&self, index: usize) -> &Vec<u8> {
        match self {
            Self::Homogeneous(labels) => labels.get_index(index).unwrap(),
            Self::Heterogeneous { y, .. } => y.get_index(index).unwrap(),
        }
    }
}

static USAGE: &str = "
Convert CSV data to matrix data.

Supported modes:
    adj   - convert a column of sources & a column of targets into
            an adjacency matrix.
    count - convert a pair of columns into a full count matrix (a bipartite
            adjacency matrix, or co-occurrence matrix, if you will).
    corr  - convert a selection of columns into a full
            correlation matrix.
    bivar - convert x & y columns into a bivariate distribution matrix (e.g. a 
            discretized scatterplot).

Note that the difference between the `adj` and `count` mode is that `count`
considers its `x` & `y` labels as two separate sets while `adj` considers `source`
and `target` labels as parts of the same set. This also means `adj` produces a
square matrix while `count` produces a rectangular one.

Usage:
    xan matrix adj [options] <source> <target> [<input>]
    xan matrix count [options] <x> <y> [<input>]
    xan matrix corr [options] [<input>]
    xan matrix bivar [options] <x> <y> [<input>]
    xan matrix --help

matrix adj/count/bivar options:
    -w, --weight <column>  Optional column containing numbers that will be used
                           as matrix cell weights, instead of just counting
                           occurrences.

matrix adj options:
    -U, --undirected  Indicates that edges are undirected and that produced
                      matrix should be symmetric.

matrix corr options:
    -s, --select <columns>  Columns to consider for the correlation
                            matrix.
    -D, --fill-diagonal     Whether to fill diagonal with ones.

matrix bivar options:
    -b, --bins <nb_columns>  Number of columns (= number of rows) to consider 
                             for the square matrix [default: 10]
    --x-bins <nb_columns>    Number of columns to consider for the matrix
                             Default to --bins.
    --y-bins <nb_rows>       Number of rows to consider for the matrix
                             Default to --bins.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
";

fn try_parse_float(value: &[u8]) -> Result<f64, String> {
    fast_float::parse::<f64, &[u8]>(value).map_err(|_| {
        format!(
            "could not parse cell \"{}\" as a float!",
            bstr::BStr::new(value)
        )
    })
}

fn get_min<T: PartialOrd>(opt_min: Option<T>, new_val: T) -> Option<T> {
    Some(match opt_min {
        Some(min_val) => {
            if new_val < min_val {
                new_val
            } else {
                min_val
            }
        }
        None => new_val,
    })
}

fn get_max<T: PartialOrd>(opt_max: Option<T>, new_val: T) -> Option<T> {
    Some(match opt_max {
        Some(max_value) => {
            if max_value < new_val {
                new_val
            } else {
                max_value
            }
        }
        None => new_val,
    })
}

#[derive(Deserialize, Debug)]
struct Args {
    cmd_adj: bool,
    cmd_count: bool,
    cmd_corr: bool,
    cmd_bivar: bool,
    arg_input: Option<String>,
    arg_x: Option<SelectedColumns>,
    arg_y: Option<SelectedColumns>,
    arg_source: Option<SelectedColumns>,
    arg_target: Option<SelectedColumns>,
    flag_weight: Option<SelectedColumns>,
    flag_select: SelectedColumns,
    flag_undirected: bool,
    flag_fill_diagonal: bool,
    flag_bins: usize,
    flag_x_bins: Option<usize>,
    flag_y_bins: Option<usize>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

impl Args {
    fn adj_or_count(self) -> CliResult<()> {
        if self.cmd_count && self.flag_undirected {
            Err("-U/--undirected does not make sense with `count` mode!")?;
        }

        let rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers);

        let mut reader = rconf.simd_reader()?;
        let headers = reader.byte_headers()?;

        let arg_source = self.arg_source.as_ref().or(self.arg_x.as_ref()).unwrap();
        let arg_target = self.arg_target.as_ref().or(self.arg_y.as_ref()).unwrap();

        let source_column_index = arg_source.single_selection(headers, !rconf.no_headers)?;
        let target_column_index = arg_target.single_selection(headers, !rconf.no_headers)?;

        let weight_column_index = self
            .flag_weight
            .as_ref()
            .map(|weight_col| weight_col.single_selection(headers, !rconf.no_headers))
            .transpose()?;

        let mut axes = Axes::new(self.cmd_count);
        let mut hash_matrix: HashMap<(usize, usize), f64> = HashMap::new();

        let mut input_record = ByteRecord::new();

        while reader.read_byte_record(&mut input_record)? {
            let mut source = input_record[source_column_index].to_vec();
            let mut target = input_record[target_column_index].to_vec();

            if self.flag_undirected && source > target {
                std::mem::swap(&mut source, &mut target);
            }

            let weight = match weight_column_index {
                Some(index) => try_parse_float(&input_record[index])?,
                None => 1.0,
            };

            let source_idx = axes.insert_x(source);
            let target_idx = axes.insert_y(target);

            hash_matrix
                .entry((source_idx, target_idx))
                .and_modify(|key| *key += weight)
                .or_insert(weight);
        }

        let (cols, rows) = axes.shape();

        let mut writer = Config::new(&self.flag_output).simd_writer()?;
        let mut output_record = ByteRecord::new();
        output_record.push_field(b"");

        for value in axes.x_labels() {
            output_record.push_field(value);
        }

        writer.write_byte_record(&output_record)?;

        let mut flat_matrix: Vec<Option<f64>> = vec![None; cols * rows];

        for ((x, y), val) in hash_matrix.iter() {
            let index = y * cols + x;
            flat_matrix[index] = Some(*val);

            if self.flag_undirected {
                // NOTE: no need to have a special case for diagonal.
                let other_index = x * cols + y;
                flat_matrix[other_index] = Some(*val);
            }
        }

        for (index, row) in flat_matrix.chunks_exact(cols).enumerate() {
            let row_label = axes.get_y_label(index);
            output_record.clear();
            output_record.push_field(row_label);

            for v_opt in row {
                match v_opt {
                    Some(v) => output_record.push_field(v.to_string().as_bytes()),
                    None => output_record.push_field(b""),
                };
            }

            writer.write_byte_record(&output_record)?;
        }

        Ok(())
    }

    fn correlation(self) -> CliResult<()> {
        let rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.flag_select.clone());

        let mut reader = rconf.simd_reader()?;
        let headers = reader.byte_headers()?.clone();

        let sel = rconf.selection(&headers)?;

        if sel.len() < 2 {
            Err("less that 2 columns in selection!")?;
        }

        let mut writer = Config::new(&self.flag_output).simd_writer()?;
        let mut output_headers = ByteRecord::new();
        output_headers.push_field(b"");
        output_headers.extend(sel.select(&headers));

        writer.write_byte_record(&output_headers)?;

        let n = sel.len();
        let m = (n * (n - 1)) / 2;

        for i in 0..n {
            let mut row: Vec<Option<CovarianceWelford>> = Vec::with_capacity(n);

            for j in 0..n {
                if i == j {
                    row.push(None);
                }
            }
        }

        let mut welfords: Vec<CovarianceWelford> = Vec::with_capacity(m);

        for _ in 0..m {
            welfords.push(CovarianceWelford::new());
        }

        let mut record = ByteRecord::new();
        let mut k: usize;

        while reader.read_byte_record(&mut record)? {
            let values = sel
                .select(&record)
                .map(|cell| {
                    fast_float::parse::<f64, &[u8]>(cell).map_err(|_| {
                        format!(
                            "could not parse cell \"{}\" as a float!",
                            std::str::from_utf8(cell).unwrap()
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            k = 0;

            for i in 0..n {
                for j in (i + 1)..n {
                    if i == j {
                        continue;
                    }

                    welfords[k].add(values[i], values[j]);

                    k += 1;
                }
            }
        }

        let mut correlation_matrix: Vec<Vec<Option<f64>>> = Vec::new();

        for _ in 0..n {
            correlation_matrix.push(vec![None; n]);
        }

        k = 0;

        for i in 0..n {
            for j in (i + 1)..n {
                let correlation = welfords[k].correlation();

                correlation_matrix[i][j] = correlation;
                correlation_matrix[j][i] = correlation;

                k += 1;
            }
        }

        for (row, name) in correlation_matrix
            .into_iter()
            .zip(output_headers.iter().skip(1))
        {
            record.clear();
            record.push_field(name);

            for cell in row {
                match cell {
                    None => record.push_field(if self.flag_fill_diagonal { b"1.0" } else { b"" }),
                    Some(f) => record.push_field(f.to_string().as_bytes()),
                }
            }

            writer.write_byte_record(&record)?;
        }

        Ok(())
    }

    fn bivar(self) -> CliResult<()> {
        let rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers);

        let mut reader = rconf.simd_reader()?;
        let headers = reader.byte_headers()?;

        let x_column_index = self
            .arg_x
            .as_ref()
            .unwrap()
            .single_selection(headers, !rconf.no_headers)?;

        let y_column_index = self
            .arg_y
            .as_ref()
            .unwrap()
            .single_selection(headers, !rconf.no_headers)?;

        let weight_column_index = self
            .flag_weight
            .as_ref()
            .map(|weight_col| weight_col.single_selection(headers, !rconf.no_headers))
            .transpose()?;

        let mut input_record = ByteRecord::new();
        let mut points = Vec::new();

        let (mut min_x, mut max_x): (Option<f64>, Option<f64>) = (None, None);
        let (mut min_y, mut max_y): (Option<f64>, Option<f64>) = (None, None);

        while reader.read_byte_record(&mut input_record)? {
            let x_value = try_parse_float(&input_record[x_column_index])?;
            let y_value = try_parse_float(&input_record[y_column_index])?;

            let weight = match weight_column_index {
                Some(index) => try_parse_float(&input_record[index])?,
                None => 1.0,
            };

            (min_x, max_x) = (get_min(min_x, x_value), get_max(max_x, x_value));
            (min_y, max_y) = (get_min(min_y, y_value), get_max(max_y, y_value));

            points.push((x_value, y_value, weight));
        }

        let (min_x, max_x) = (min_x.unwrap(), max_x.unwrap());
        let (min_y, max_y) = (min_y.unwrap(), max_y.unwrap());

        let nb_cols = match self.flag_x_bins {
            Some(nb_bins) => nb_bins,
            None => self.flag_bins,
        };
        let nb_rows = match self.flag_y_bins {
            Some(nb_bins) => nb_bins,
            None => self.flag_bins,
        };

        let cell_width: f64 = (max_x - min_x) / (nb_cols as f64);
        let cell_height: f64 = (max_y - min_y) / (nb_rows as f64);

        let mut x_labels = Vec::with_capacity(nb_cols);
        let mut x_lower_bound = min_x;
        for _ in 0..(nb_cols - 1) {
            x_labels.push(format!(
                ">= {} < {}",
                util::format_number(x_lower_bound),
                util::format_number(x_lower_bound + cell_width)
            ));
            x_lower_bound += cell_width;
        }
        x_labels.push(format!(
            ">= {} <= {}",
            util::format_number(x_lower_bound),
            util::format_number(max_x)
        ));

        let mut y_labels = Vec::with_capacity(nb_rows);
        let mut y_lower_bound = min_y;
        for _ in 0..(nb_cols - 1) {
            y_labels.push(format!(
                ">= {} < {}",
                util::format_number(y_lower_bound),
                util::format_number(y_lower_bound + cell_height)
            ));
            y_lower_bound += cell_height;
        }
        y_labels.push(format!(
            ">= {} <= {}",
            util::format_number(y_lower_bound),
            util::format_number(max_y)
        ));

        let mut writer = Config::new(&self.flag_output).simd_writer()?;
        let mut output_record = ByteRecord::new();
        output_record.push_field(b"");

        for value in x_labels {
            output_record.push_field(value.as_bytes());
        }

        writer.write_byte_record(&output_record)?;

        let mut flat_matrix: Vec<Option<f64>> = vec![None; nb_cols * nb_rows];

        let list_points = points.into_iter();
        for (x_value, y_value, weight) in list_points {
            let idx_col = (((x_value - min_x) / cell_width).floor() as usize).min(nb_cols - 1);
            let idx_row = (((y_value - min_y) / cell_height).floor() as usize).min(nb_rows - 1);

            flat_matrix[idx_row + nb_rows * idx_col] =
                Some(match flat_matrix[idx_row + nb_rows * idx_col] {
                    Some(value) => value + weight,
                    None => weight,
                })
        }

        for (row, label) in flat_matrix.chunks_exact(nb_cols).zip(&y_labels) {
            let row_label = label.as_bytes();
            output_record.clear();
            output_record.push_field(row_label);

            for v_opt in row {
                match v_opt {
                    Some(v) => output_record.fmt_field(&v),
                    None => output_record.push_field(b""),
                };
            }

            writer.write_byte_record(&output_record)?;
        }
        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_adj || args.cmd_count {
        args.adj_or_count()
    } else if args.cmd_corr {
        args.correlation()
    } else if args.cmd_bivar {
        args.bivar()
    } else {
        unreachable!()
    }
}
