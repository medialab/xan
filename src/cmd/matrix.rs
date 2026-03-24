use crate::config::{Config, Delimiter};
use crate::moonblade::agg::CovarianceWelford;
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;
use crate::collections::HashMap;
use indexmap::set::IndexSet;

static USAGE: &str = "
Convert CSV data to matrix data.

Supported modes:
    count   - convert a pair of columns into a full count
            matrix.
    corr    - convert a selection of columns into a full
            correlation matrix.

Usage:
    xan matrix count [options] <source> <target> [<input>]
    xan matrix corr [options] [<input>]
    xan matrix --help

matrix adj options:
    -w, --weight <column>  Optional column containing a weight for edges.

matrix corr options:
    -s, --select <columns>  Columns to consider for the correlation
                            matrix.
    -D, --fill-diagonal     Whether to fill diagonal with ones.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_input: Option<String>,
    arg_source: Option<SelectedColumns>,
    arg_target: Option<SelectedColumns>,
    cmd_count: bool,
    cmd_corr: bool,
    flag_weight: Option<SelectedColumns>,
    flag_select: SelectedColumns,
    flag_fill_diagonal: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

impl Args {
    fn count(self) -> CliResult<()> {
        let rconf = Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.flag_select.clone());

        let mut reader = rconf.simd_reader()?;
        let headers = reader.byte_headers()?;

        let arg_source = self.arg_source.as_ref().unwrap();
        let arg_target = self.arg_target.as_ref().unwrap();

        let source_column_index = arg_source.single_selection(headers, !rconf.no_headers)?;
        let target_column_index = arg_target.single_selection(headers, !rconf.no_headers)?;
        let weight_column_index = match self.flag_weight.as_ref() {
            Some(column) => Some(column.single_selection(headers, !rconf.no_headers)?),
            None => None,
        };

        let mut source_set = IndexSet::new();
        let mut target_set = IndexSet::new();
        let mut hash_matrix = HashMap::new();

        let mut input_record = simd_csv::ByteRecord::new();

        while reader.read_byte_record(&mut input_record)? {
            let source_cell = input_record[source_column_index].to_vec();
            let target_cell = input_record[target_column_index].to_vec();

            let weight = match weight_column_index {
                Some(index) => {
                    let weight_str = &input_record[index];

                    fast_float::parse::<f64, &[u8]>(weight_str)
                        .map_err(|_| {
                            format!(
                                "could not parse cell \"{}\" as a float!",
                                std::str::from_utf8(weight_str).unwrap()
                            )
                        })?
                }
                None => 1.0,
            };

            let (source_idx, _) = source_set.insert_full(source_cell.clone());
            let (target_idx, _) = target_set.insert_full(target_cell.clone());

            let tuple = (source_idx, target_idx);

            hash_matrix
                .entry(tuple)
                .and_modify(|key|  *key += weight)
                .or_insert(weight);

        }

        let mut writer = Config::new(&self.flag_output).simd_writer()?;
        let mut output_record = simd_csv::ByteRecord::new();
        output_record.push_field(b"");

        for value in target_set.iter() {
            let label = value.as_slice();
            output_record.push_field(label);
        }

        writer.write_byte_record(&output_record)?;

        let mut values_vector = vec![0.0; source_set.len() * target_set.len()];

        for (key, val) in hash_matrix.iter() {
            let (coord_source, coord_target) = key;
            
            let index = coord_source * target_set.len() + coord_target;
            values_vector[index] = *val;
        }

        let row_size = source_set.len();
        for (index, window) in values_vector.chunks_exact(row_size).enumerate() {
            let row_label = source_set[index].clone();
            output_record.clear();
            output_record.push_field(&row_label);
            
            for v in window {
                output_record.push_field(v.to_string().as_bytes());
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
        let mut output_headers = simd_csv::ByteRecord::new();
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

        let mut record = simd_csv::ByteRecord::new();
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
                    None => record.push_field(if self.flag_fill_diagonal { b"1" } else { b"" }),
                    Some(f) => record.push_field(f.to_string().as_bytes()),
                }
            }

            writer.write_byte_record(&record)?;
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_count {
        args.count()
    } else if args.cmd_corr {
        args.correlation()
    } else {
        unreachable!()
    }
}
