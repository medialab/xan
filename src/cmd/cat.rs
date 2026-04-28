use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Concatenates CSV data by column or by row.

When concatenating by column, the columns will be written in the same order as
the inputs given. The number of rows in the result is always equivalent to to
the minimum number of rows across all given CSV data. (This behavior can be
reversed with the '--pad' flag.)

When concatenating by row, all CSV data must have the same number of columns.
If you need to rearrange the columns or fix the lengths of records, use the
'select' or 'fixlengths' commands. Also, only the headers of the *first* CSV
data given are used. Headers in subsequent inputs are ignored. (This behavior
can be disabled with --no-headers.)

When concatenating a large number of CSV files exceeding your shell's
command arguments limit, prefer using the --paths flag to read the list of CSV
files to concatenate from input lines or from a CSV file containing paths in a
column given to the --path-column flag.

Feeding --paths lines:

    $ xan cat rows --paths paths.txt > concatenated.csv

Feeding --paths CSV file:

    $ xan cat rows --paths files.csv --path-column path > concatenated.csv

Feeding stdin (\"-\") to --paths:

    $ find . -name '*.csv' | xan cat rows --paths - > concatenated.csv

Feeding CSV as stdin (\"-\") to --paths:

    $ cat filelist.csv | xan cat rows --paths - --path-column path > concatenated.csv

Usage:
    xan cat rows [options] [<inputs>...]
    xan cat (cols|columns) [options] [<inputs>...]
    xan cat --help

cat cols/columns options:
    -p, --pad                   When concatenating columns, this flag will cause
                                all records to appear. It will pad each row if
                                other CSV data isn't long enough.

cat rows options:
    --paths <input>             When concatenating rows, give a text file (use \"-\" for stdin)
                                containing one path of CSV file to concatenate per line.
    --path-column <name>        When given a column name, --paths will be considered as CSV, and paths
                                to CSV files to concatenate will be extracted from the selected column.
    -S, --source-column <name>  Name of a column to prepend in the output of \"cat rows\"
                                indicating the path to source file.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    cmd_rows: bool,
    cmd_columns: bool,
    cmd_cols: bool,
    arg_inputs: Vec<String>,
    flag_paths: Option<String>,
    flag_path_column: Option<SelectedColumns>,
    flag_pad: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_source_column: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_paths.is_some() && !args.arg_inputs.is_empty() {
        Err("--paths cannot be used with other positional arguments!")?;
    }

    if args.cmd_rows {
        args.cat_rows()
    } else if args.cmd_columns || args.cmd_cols {
        args.cat_columns()
    } else {
        unreachable!();
    }
}

impl Args {
    fn paths(&self) -> CliResult<Box<dyn Iterator<Item = CliResult<String>>>> {
        if let Some(paths_path) = self.flag_paths.as_ref() {
            Config::new(&Some(paths_path.clone())).lines(&self.flag_path_column)
        } else {
            Ok(Box::new(self.arg_inputs.clone().into_iter().map(Ok)))
        }
    }

    fn cat_rows(&self) -> CliResult<()> {
        let mut record = ByteRecord::new();
        let mut wtr = Config::new(&self.flag_output).simd_writer()?;

        let mut headers_opt: Option<ByteRecord> = None;

        for result in self.paths()? {
            let path = result?;

            let mut reader = Config::new(&Some(path.clone()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers)
                .simd_reader()?;

            if reader.has_headers() {
                match &headers_opt {
                    Some(headers) => {
                        let current_headers = reader.byte_headers()?;

                        if current_headers != headers {
                            Err(format!("found inconsistent headers as soon as path \"{}\"!\nExpected: {:?}\nGot: {:?}", path, headers, current_headers))?;
                        }
                    }
                    None => {
                        let current_headers = reader.byte_headers()?;

                        headers_opt = Some(current_headers.clone());

                        if let Some(source_column) = &self.flag_source_column {
                            wtr.write_record(
                                [source_column.as_bytes()]
                                    .into_iter()
                                    .chain(current_headers.iter()),
                            )?;
                        } else {
                            wtr.write_byte_record(current_headers)?;
                        }
                    }
                }
            } else {
                match &headers_opt {
                    Some(headers) => {
                        let current_headers = reader.byte_headers()?;

                        if headers.len() != current_headers.len() {
                            Err(format!("found inconsistent column count as soon as path \"{}\"!\nExpected: {}\nGot: {}", path, headers.len(), current_headers.len()))?;
                        }
                    }
                    None => {
                        headers_opt = Some(reader.byte_headers()?.clone());
                    }
                }
            }

            if self.flag_source_column.is_none() {
                while reader.read_byte_record(&mut record)? {
                    wtr.write_byte_record(&record)?;
                }
            } else {
                while reader.read_byte_record(&mut record)? {
                    wtr.write_record([path.as_bytes()].into_iter().chain(&record))?;
                }
            }
        }

        Ok(wtr.flush()?)
    }

    fn cat_columns(&self) -> CliResult<()> {
        let mut wtr = Config::new(&self.flag_output).simd_writer()?;

        let mut rdrs = self
            .paths()?
            .map(|p| -> CliResult<_> {
                Config::new(&Some(p?))
                    .delimiter(self.flag_delimiter)
                    .no_headers(true) // NOTE: header info is irrelevant for this operation
                    .simd_reader()
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Find the lengths of each record. If a length varies, then an error
        // will occur so we can rely on the first length being the correct one.
        let mut lengths = vec![];

        for rdr in &mut rdrs {
            lengths.push(rdr.byte_headers()?.len());
        }

        let mut iters = rdrs
            .iter_mut()
            .map(|rdr| rdr.byte_records())
            .collect::<Vec<_>>();

        'OUTER: loop {
            let mut record = ByteRecord::new();
            let mut num_done = 0;
            for (iter, &len) in iters.iter_mut().zip(lengths.iter()) {
                match iter.next() {
                    None => {
                        num_done += 1;
                        if self.flag_pad {
                            for _ in 0..len {
                                record.push_field(b"");
                            }
                        } else {
                            break 'OUTER;
                        }
                    }
                    Some(Err(err)) => Err(err)?,
                    Some(Ok(next)) => record.extend(&next),
                }
            }
            // Only needed when `--pad` is set.
            // When not set, the OUTER loop breaks when the shortest iterator
            // is exhausted.
            if num_done >= iters.len() {
                break 'OUTER;
            }
            wtr.write_byte_record(&record)?;
        }
        wtr.flush().map_err(From::from)
    }
}
