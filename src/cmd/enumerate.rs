use std::io::{self, Write};

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Enumerate a CSV file by preprending an index column to each row.

Alternatively prepend a byte offset column instead when using
the -B, --byte-offset flag.

Usage:
    xan enum [options] [<input>]
    xan enum --help

enum options:
    -c, --column-name <arg>  Name of the column to prepend. Will default to \"index\",
                             or \"byte_offset\" when -B, --byte-offset is given.
    -S, --start <arg>        Number to count from. [default: 0].
    -B, --byte-offset        Whether to indicate the byte offset of the row
                             in the file instead. Can be useful to perform
                             constant time slicing with `xan slice --byte-offset`
                             later on.
    -A, --accumulate         When use with -B, --byte-offset, will accumulate the
                             written offset size in bytes to create an autodescriptive
                             file that can be seen as a means of indexation.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not considered as being
                           the file header.
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_start: i64,
    flag_column_name: Option<String>,
    flag_byte_offset: bool,
    flag_accumulate: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let delimiter = conf.delimiter;

    let mut splitter = conf.simd_splitter()?;
    let mut wtr = Config::new(&args.flag_output).buf_io_writer()?;

    let mut write = |value: &[u8], record: &[u8]| -> io::Result<()> {
        wtr.write_all(value)?;
        wtr.write_all(&[delimiter])?;
        wtr.write_all(record)?;
        wtr.write_all(b"\n")?;

        Ok(())
    };

    if !conf.no_headers {
        let column_name = args.flag_column_name.unwrap_or(
            (if args.flag_byte_offset {
                "byte_offset"
            } else {
                "index"
            })
            .to_string(),
        );

        if let Some(headers) = splitter.split_record()? {
            write(column_name.as_bytes(), headers)?;
        }
    }

    let mut counter = args.flag_start;
    let mut accumulator: u64 = 0;
    let mut pos: u64 = splitter.position();

    while let Some(record) = splitter.split_record()? {
        if args.flag_byte_offset {
            let offset = (pos + accumulator).to_string();

            if args.flag_accumulate {
                accumulator += offset.len() as u64 + 1;
            }

            write(offset.as_bytes(), record)?;
        } else {
            write(counter.to_string().as_bytes(), record)?;
        }

        pos = splitter.position();
        counter += 1;
    }

    Ok(wtr.flush()?)
}
