use std::path::PathBuf;

use csv;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliResult;

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

Usage:
    xsv cat rows <column> -i <input> [options]
    xsv cat rows    [options] [<inputs>...]
    xsv cat columns [options] [<inputs>...]
    xsv cat --help

cat options:
    -p, --pad              When concatenating columns, this flag will cause
                           all records to appear. It will pad each row if
                           other CSV data isn't long enough.
    -i, --input <input>    When concatenating rows, path to a CSV file containing
                           a column of paths to other CSV files to concatenate.
    -I, --input-dir <dir>  When concatenating rows, root directory to resolve
                           relative paths contained in the -i/--input file column.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    cmd_rows: bool,
    cmd_columns: bool,
    arg_inputs: Vec<String>,
    arg_column: Option<SelectColumns>,
    flag_input: Option<String>,
    flag_input_dir: Option<PathBuf>,
    flag_pad: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_rows {
        if args.arg_column.is_some() {
            args.cat_rows_with_input()
        } else {
            args.cat_rows()
        }
    } else if args.cmd_columns {
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
        let mut row = csv::ByteRecord::new();
        let mut wtr = Config::new(&self.flag_output).writer()?;
        for (i, conf) in self.configs()?.into_iter().enumerate() {
            let mut rdr = conf.reader()?;
            if i == 0 {
                conf.write_headers(&mut rdr, &mut wtr)?;
            }
            while rdr.read_byte_record(&mut row)? {
                wtr.write_byte_record(&row)?;
            }
        }
        wtr.flush().map_err(From::from)
    }

    fn cat_rows_with_input(&self) -> CliResult<()> {
        let rconf = Config::new(&self.flag_input)
            .delimiter(self.flag_delimiter)
            .select(self.arg_column.clone().unwrap());

        let mut rdr = rconf.reader()?;
        let headers = rdr.byte_headers()?;

        let column_index = rconf.single_selection(&headers)?;

        let mut record = csv::StringRecord::new();
        let mut sub_record = csv::ByteRecord::new();

        let mut wtr = Config::new(&self.flag_output).writer()?;

        let mut headers_written = self.flag_no_headers;

        while rdr.read_record(&mut record)? {
            let mut path = record[column_index].to_string();

            if let Some(root_dir) = &self.flag_input_dir {
                let mut buf = root_dir.clone();
                buf.push(path);
                path = buf.to_string_lossy().into_owned();
            }

            let sub_rconf = Config::new(&Some(path))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let mut sub_rdr = sub_rconf.reader()?;

            if !headers_written {
                let headers = sub_rdr.byte_headers()?;
                headers_written = true;
                wtr.write_byte_record(&headers)?;
            }

            while sub_rdr.read_byte_record(&mut sub_record)? {
                wtr.write_byte_record(&sub_record)?;
            }
        }

        Ok(wtr.flush()?)
    }

    fn cat_columns(&self) -> CliResult<()> {
        let mut wtr = Config::new(&self.flag_output).writer()?;
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
            let mut record = csv::ByteRecord::new();
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
                    Some(Err(err)) => return fail!(err),
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
