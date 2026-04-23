use std::fs;

use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::moonblade::Program;
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
The transform command can be used to edit a selection of columns for each row
of a CSV file using a custom expression.

For instance, given the following CSV file:

name,surname
john,davis
mary,sue

The following command (notice how `_` is used as a reference to the currently
edited column):

    $ xan transform surname 'upper(_)'

Will produce the following result:

name,surname
john,DAVIS
mary,SUE

When using unary functions, the above command can be written even shorter:

    $ xan transform surname upper

The above example work on a single column but the command is perfectly able to
transform multiple columns at once using a selection:

    $ xan transform name,surname,fullname upper

The expression can optionally be read from a file using the -f/--evaluate-file flag:

    $ xan transform name -f expr.moonblade file.csv > result.csv

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan transform [options] <column> <expression> [<input>]
    xan transform --help

transform options:
    -f, --evaluate-file        Read evaluation expression from a file instead.
    -r, --rename <name>        New name for the transformed column.
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize)]
struct Args {
    arg_column: SelectedColumns,
    arg_expression: String,
    arg_input: Option<String>,
    flag_rename: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_evaluate_file: bool,
}

impl Args {
    fn resolve(&mut self) -> CliResult<()> {
        if self.flag_evaluate_file {
            self.arg_expression = fs::read_to_string(&self.arg_expression)?;
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve()?;

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter)
        .select(args.arg_column);

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let mut sel = rconf.selection(&headers)?;
    sel.dedup();

    let mask = sel.mask(headers.len());

    let program = Program::parse(&args.arg_expression, &headers, rconf.no_headers)?;

    if !rconf.no_headers {
        let output_headers = if let Some(new_names) = &args.flag_rename {
            let renamed = util::str_to_csv_byte_record(new_names);

            if renamed.len() != sel.len() {
                Err(format!(
                    "Renamed columns alignment error. Expected {} names and got {}.",
                    sel.len(),
                    renamed.len(),
                ))?;
            }

            sel.indexed_mask(headers.len())
                .iter()
                .zip(headers.iter())
                .map(|(o, h)| if let Some(i) = o { &renamed[*i] } else { h })
                .collect()
        } else {
            headers.clone()
        };

        wtr.write_record(&output_headers)?;
    }

    if let Some(threads) = parallelization {
        for result in rdr.into_byte_records().enumerate().parallel_map_custom(
            |o| o.threads(threads.unwrap_or_else(crate::util::default_num_cpus)),
            move |(index, record)| -> CliResult<simd_csv::ByteRecord> {
                let record = record?;

                let mut output_record = simd_csv::ByteRecord::new();

                for (col_index, do_transform) in mask.iter().copied().enumerate() {
                    if do_transform {
                        let value =
                            program.run_with_record_and_col_index(index, col_index, &record)?;

                        output_record.push_field(&value.serialize_as_bytes());
                    } else {
                        output_record.push_field(&record[col_index]);
                    }
                }

                Ok(output_record)
            },
        ) {
            wtr.write_byte_record(&result?)?;
        }
    } else {
        let mut record = simd_csv::ByteRecord::new();
        let mut output_record = simd_csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            output_record.clear();

            for (col_index, do_transform) in mask.iter().copied().enumerate() {
                if do_transform {
                    let value = program.run_with_record_and_col_index(index, col_index, &record)?;

                    output_record.push_field(&value.serialize_as_bytes());
                } else {
                    output_record.push_field(&record[col_index]);
                }
            }

            wtr.write_byte_record(&output_record)?;

            index += 1;
        }
    }

    Ok(wtr.flush()?)
}
