use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
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
    flag_path_column: Option<SelectColumns>,
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
        if args.flag_paths.is_some() {
            args.cat_rows_with_input()
        } else {
            args.cat_rows()
        }
    } else if args.cmd_columns || args.cmd_cols {
        args.cat_columns()
    } else {
        unreachable!();
    }
}

impl Args {
    fn configs(&self) -> CliResult<Vec<Config>> {
        util::many_configs(
            &self.arg_inputs,
            self.flag_delimiter,
            self.flag_no_headers,
            None,
        )
        .map_err(From::from)
    }

    fn cat_rows(&self) -> CliResult<()> {
        let mut row = simd_csv::ByteRecord::new();
        let mut wtr = Config::new(&self.flag_output).simd_writer()?;
        for (i, conf) in self.configs()?.into_iter().enumerate() {
            let mut rdr = conf.simd_reader()?;

            match &self.flag_source_column {
                None => {
                    if !conf.no_headers && i == 0 {
                        wtr.write_byte_record(rdr.byte_headers()?)?;
                    }
                    while rdr.read_byte_record(&mut row)? {
                        wtr.write_byte_record(&row)?;
                    }
                }
                Some(source_column) => {
                    if !conf.no_headers && i == 0 {
                        let headers = rdr.byte_headers()?;
                        wtr.write_record([source_column.as_bytes()].into_iter().chain(headers))?;
                    }

                    let source = conf
                        .path
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or("<stdin>".to_string());

                    while rdr.read_byte_record(&mut row)? {
                        wtr.write_record([source.as_bytes()].into_iter().chain(&row))?;
                    }
                }
            }
        }
        wtr.flush().map_err(From::from)
    }

    fn cat_rows_with_input(&self) -> CliResult<()> {
        let paths =
            Config::new(&Some(self.flag_paths.clone().unwrap())).lines(&self.flag_path_column)?;

        let mut record = simd_csv::ByteRecord::new();
        let mut wtr = Config::new(&self.flag_output).simd_writer()?;

        let mut headers_written = self.flag_no_headers;

        for result in paths {
            let path = result?;

            let mut reader = Config::new(&Some(path.clone()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers)
                .simd_reader()?;

            match &self.flag_source_column {
                None => {
                    if !headers_written {
                        let headers = reader.byte_headers()?;
                        wtr.write_byte_record(headers)?;
                        headers_written = true;
                    }

                    while reader.read_byte_record(&mut record)? {
                        wtr.write_byte_record(&record)?;
                    }
                }
                Some(source_column) => {
                    if !headers_written {
                        let headers = reader.byte_headers()?;
                        wtr.write_record(
                            [source_column.as_bytes()].into_iter().chain(headers.iter()),
                        )?;
                        headers_written = true;
                    }

                    while reader.read_byte_record(&mut record)? {
                        wtr.write_record([path.as_bytes()].into_iter().chain(&record))?;
                    }
                }
            }
        }

        Ok(wtr.flush()?)
    }

    fn cat_columns(&self) -> CliResult<()> {
        let mut wtr = Config::new(&self.flag_output).simd_writer()?;
        let mut rdrs = self
            .configs()?
            .into_iter()
            .map(|conf| conf.no_headers(true).reader())
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
            let mut record = simd_csv::ByteRecord::new();
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
