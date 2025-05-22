use std::io::{Read, SeekFrom};

use crate::config::{Config, Delimiter};
use crate::read::read_byte_record_up_to;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Returns the rows in the range specified (starting at 0, half-open interval).
The range does not include headers.

If the start of the range isn't specified, then the slice starts from the first
record in the CSV data.

If the end of the range isn't specified, then the slice continues to the last
record in the CSV data.

Finally, this command is also able to find the first record to slice in
constant time using the -B, --byte-offset if you know its byte offset in
the file. This only works with seekable inputs, e.g. files but no stdin or
gzipped files.

TODO: explain how byte & conditions affect

Usage:
    xan slice [options] [<input>]

slice options:
    -s, --start <n>    The index of the record to slice from.
    --skip <n>         Same as -s, --start.
    -e, --end <n>      The index of the record to slice to.
    -l, --len <n>      The length of the slice (can be used instead of --end).
    -i, --index <i>    Slice a single record (shortcut for -s N -l 1).
    -I, --indices <i>  Return a slice containing multiple indices at once.
                       You must provide the indices separated by commas,
                       e.g. \"1,4,67,89\". Note that selected records will be
                       emitted in file order, not in the order given.

slice options related to byte offets:
    -B, --byte-offset <b>  Byte offset to seek to in the sliced file. This can
                           be useful to access a particular slice of records in
                           constant time, without needing to read preceding bytes.
                           You must provide a byte offset starting a CSV record or
                           the output could be corrupted. This requires the input
                           to be seekable (stdin or gzipped files not supported).
    -E, --end-byte <b>     Only read up to provided position in byte, exclusive.
                           This requires the input to be seekable (stdin or gzipped
                           files not supported).

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Otherwise, the first row will always
                           appear in the output as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_start: Option<usize>,
    flag_skip: Option<usize>,
    flag_end: Option<usize>,
    flag_len: Option<usize>,
    flag_index: Option<usize>,
    flag_indices: Option<String>,
    flag_byte_offset: Option<u64>,
    flag_end_byte: Option<u64>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn resolve(&mut self) {
        if let (None, Some(skip)) = (self.flag_start, self.flag_skip) {
            self.flag_start = Some(skip);
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    if args.flag_indices.is_some() {
        return {
            let rconf = args.rconfig();

            if let Some(offset) = args.flag_byte_offset {
                let inner = rconf.io_reader_for_random_access()?;
                let mut rdr = rconf.csv_reader_from_reader(inner);

                let mut pos = csv::Position::new();
                pos.set_byte(offset);

                rdr.seek_raw(SeekFrom::Start(offset), pos)?;

                args.run_plural(rdr)
            } else {
                let rdr = rconf.reader()?;
                args.run_plural(rdr)
            }
        };
    }

    let rconf = args.rconfig();

    if let Some(offset) = args.flag_byte_offset {
        let inner = rconf.io_reader_for_random_access()?;
        let mut rdr = rconf.csv_reader_from_reader(inner);

        let mut pos = csv::Position::new();
        pos.set_byte(offset);

        rdr.seek_raw(SeekFrom::Start(offset), pos)?;

        args.run(rdr)
    } else {
        let rdr = rconf.reader()?;
        args.run(rdr)
    }
}

impl Args {
    fn run<R: Read>(&self, mut rdr: csv::Reader<R>) -> CliResult<()> {
        let mut wtr = self.wconfig().writer()?;
        self.rconfig().write_headers(&mut rdr, &mut wtr)?;

        let mut record = csv::ByteRecord::new();

        let (start, end) = self.range()?;
        let mut i: usize = 0;

        while read_byte_record_up_to(&mut rdr, &mut record, self.flag_end_byte)? {
            i += 1;

            if i <= start {
                continue;
            }

            wtr.write_byte_record(&record)?;

            if i == end {
                break;
            }
        }

        Ok(wtr.flush()?)
    }

    fn run_plural<R: Read>(&self, mut rdr: csv::Reader<R>) -> CliResult<()> {
        let mut wtr = self.wconfig().writer()?;
        self.rconfig().write_headers(&mut rdr, &mut wtr)?;

        let indices = self.plural_indices()?;

        let mut record = csv::ByteRecord::new();
        let mut i: usize = 0;

        while read_byte_record_up_to(&mut rdr, &mut record, self.flag_end_byte)? {
            if indices.contains(&i) {
                wtr.write_byte_record(&record)?;
            }

            i += 1;

            if &i > indices.last().unwrap() {
                break;
            }
        }

        Ok(wtr.flush()?)
    }

    fn range(&self) -> Result<(usize, usize), String> {
        util::range(
            self.flag_start,
            self.flag_end,
            self.flag_len,
            self.flag_index,
        )
    }

    // NOTE: there is room to optimize, but this seems pointless currently
    fn plural_indices(&self) -> Result<Vec<usize>, &str> {
        self.flag_indices
            .as_ref()
            .unwrap()
            .split(',')
            .map(|string| {
                string
                    .parse::<usize>()
                    .map_err(|_| "could not parse some index in -i/--index!")
            })
            .collect::<Result<Vec<usize>, _>>()
            .map(|mut indices| {
                indices.sort();
                indices
            })
    }

    fn rconfig(&self) -> Config {
        Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
    }

    fn wconfig(&self) -> Config {
        Config::new(&self.flag_output)
    }
}
