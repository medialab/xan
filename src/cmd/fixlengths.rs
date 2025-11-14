use std::cmp::Ordering;
use std::iter::repeat_n;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Transforms CSV data so that all rows have the same number of columns. The number
of columns will be inferred to be the maximum number of columns seen for any row
in the input data by default. Unfortunately, for this to work, all data must be
buffered into memory.

Else, you can also indicate a number of columns to force with the -l/--length
flag (longer rows will get truncated, shorted rows will get padded with empty
columns).

You can also use the -H/--trust-header to trust that the file's first row has
the correct number of columns.

When using -l/--length or -H/--trust-header, the data will be streamed and does
not need to be buffered into memory.

Usage:
    xan fixlengths [options] [<input>]

fixlengths options:
    -l, --length <arg>     Forcefully set the length of each record. If a
                           record is not the size given, then it is truncated
                           or padded as appropriate.
    -H, --trust-header     Trust that the first row indicates the correct
                           number of columns of the file.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_length: Option<usize>,
    flag_trust_header: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_length.is_some() && args.flag_trust_header {
        Err("-l/--length cannot be used with -H/--trust-header!")?;
    }

    let config = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(true)
        .flexible(true);

    let mut reader = config.simd_reader()?;
    let mut writer = Config::new(&args.flag_output).simd_writer()?;

    if args.flag_length.is_some() || args.flag_trust_header {
        let length = if let Some(l) = args.flag_length {
            l
        } else {
            reader.byte_headers()?.len()
        };

        let mut record = simd_csv::ByteRecord::new();

        while reader.read_byte_record(&mut record)? {
            match record.len().cmp(&length) {
                Ordering::Equal => {
                    writer.write_byte_record(&record)?;
                }
                Ordering::Greater => {
                    writer.write_record(record.iter().take(length))?;
                }
                Ordering::Less => {
                    writer.write_record(
                        record
                            .iter()
                            .chain(repeat_n("".as_bytes(), length - record.len())),
                    )?;
                }
            }
        }
    } else {
        let mut records = Vec::new();
        let mut max_length: usize = 1;

        for result in reader.into_byte_records() {
            let record = result?;

            let mut index = 0;
            let mut nonempty_count = 0;

            for field in record.iter() {
                index += 1;

                if index == 1 || !field.is_empty() {
                    nonempty_count = index;
                }
            }

            max_length = max_length.max(nonempty_count);
            records.push(record);
        }

        for record in records {
            match record.len().cmp(&max_length) {
                Ordering::Equal => {
                    writer.write_byte_record(&record)?;
                }
                Ordering::Less => {
                    writer.write_record(
                        record
                            .iter()
                            .chain(repeat_n("".as_bytes(), max_length - record.len())),
                    )?;
                }
                Ordering::Greater => {
                    writer.write_record(record.iter().take(max_length))?;
                }
            }
        }
    }

    Ok(writer.flush()?)
}
