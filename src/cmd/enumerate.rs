use std::fmt::Display;
use std::io::{self, BufWriter, Write};

use crate::CliResult;
use crate::config::{Config, Delimiter};
use crate::utils;

struct TrackingBufWriter<W: Write> {
    written: u64,
    inner: BufWriter<W>,
}

impl<W: Write> TrackingBufWriter<W> {
    fn new(inner: BufWriter<W>) -> Self {
        Self { written: 0, inner }
    }

    #[inline]
    fn prepend(&mut self, value: &[u8], delimiter: u8, record: &[u8]) -> io::Result<()> {
        self.write_all(value)?;
        self.write_all(&[delimiter])?;
        self.write_all(record)?;
        self.write_all(b"\n")
    }

    #[inline]
    fn prepend_int<V: Display>(
        &mut self,
        value: V,
        delimiter: u8,
        record: &[u8],
    ) -> io::Result<()> {
        write!(self, "{}", value)?;
        self.write_all(&[delimiter])?;
        self.write_all(record)?;
        self.write_all(b"\n")
    }
}

impl<W: Write> Write for TrackingBufWriter<W> {
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.written += written as u64;
        Ok(written)
    }

    #[inline(always)]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

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
    -A, --accumulate         Similar to -B/--byte-offset but will accumulate the
                             written offset size in bytes to create an autodescriptive
                             file that can be seen as a means of indexing the file.

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
    let args: Args = utils::get_args(USAGE, argv)?;

    if args.flag_accumulate && args.flag_byte_offset {
        Err("-B/--byte-offset is not compatible with -A/--accumulate!")?;
    }

    let column_name = match &args.flag_column_name {
        Some(name) => name,
        None => {
            if args.flag_byte_offset || args.flag_accumulate {
                "byte_offset"
            } else {
                "index"
            }
        }
    };

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let delimiter = conf.delimiter;

    let mut splitter = conf.simd_splitter()?;
    let mut wtr = TrackingBufWriter::new(Config::new(&args.flag_output).buf_io_writer()?);

    if !conf.no_headers {
        wtr.prepend(column_name.as_bytes(), delimiter, splitter.byte_headers()?)?;
    }

    let mut counter = args.flag_start;
    let mut pos: u64 = splitter.position();

    while let Some(record) = splitter.split_record()? {
        if args.flag_byte_offset {
            wtr.prepend_int(pos, delimiter, record)?;
        } else if args.flag_accumulate {
            wtr.prepend_int(wtr.written, delimiter, record)?;
        } else {
            wtr.prepend_int(counter, delimiter, record)?;
        }

        pos = splitter.position();
        counter += 1;
    }

    Ok(wtr.flush()?)
}
