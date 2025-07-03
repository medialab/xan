use std::collections::VecDeque;
use std::io::{copy, Read, SeekFrom};

use crate::config::{Config, Delimiter};
use crate::moonblade::Program;
use crate::read::read_byte_record_up_to;
use crate::util;
use crate::CliResult;

enum ControlFlow {
    Continue,
    Break,
    Process,
}

struct Conditions {
    start: Option<Program>,
    end: Option<Program>,
    has_started: bool,
}

impl Conditions {
    fn process(&mut self, index: usize, record: &csv::ByteRecord) -> CliResult<ControlFlow> {
        if !self.has_started {
            if let Some(program) = &self.start {
                let value = program.run_with_record(index, record)?;

                if value.is_truthy() {
                    self.has_started = true;
                } else {
                    return Ok(ControlFlow::Continue);
                }
            }
        }

        if let Some(program) = &self.end {
            let value = program.run_with_record(index, record)?;

            if value.is_truthy() {
                return Ok(ControlFlow::Break);
            }
        }

        Ok(ControlFlow::Process)
    }
}

static USAGE: &str = "
Returns rows of a CSV file in the specified range. This range can be specified
through 0-based rows indices, byte offsets in the file and using custom expressions
as start & stop conditions.

Slicing the 10 first rows of a file:

    $ xan slice -l 10 file.csv

Slicing rows between indices 5 and 10:

    $ xan slice -s 5 -e 10 file.csv

Retrieving rows at some indices:

    $ xan slice -I 4,5,19,65 file.csv

Retrieving last 5 rows:

    $ xan slice -L 5 file.csv

Slicing rows starting at some byte offset in the file:

    $ xan slice -B 56356 file.csv

Slicing rows until a row where the \"count\" column is over `45`:

    $ xan slice -E 'count > 45' file.csv

The command will of course terminate as soon as the specified range of rows is
found and won't need to read to whole file or stream if unnecessary.

Of course, flags related to byte offsets will only work with seekable inputs, e.g. files
on disk but no stdin nor gzipped files.

Note that it is perfectly fine to mix & match flags related to row indices,
byte offsets and conditions. In which case, here is description of the order
of operations:

- First, the command will seek in target file if -B/--byte-offset was given, and
won't read past a certain byte offset if --end-byte was given.
- Then the -S/--start-condition and -E/--end-condtion apply.
- Finally flags related to row indices will apply. Note that indices are therefore
relative to both the application of the byte offset and the start condition and not
to the first actual row in the file.

So, for instance, if you want to slice 5 rows in the file but only after a row
where the \"count\" column is over `10`, you could do the following:

    $ xan slice -S 'count > 10' -l 5 file.csv

Usage:
    xan slice [options] [<input>]

slice options to use with row indices:
    -s, --start <n>    The index of the row to slice from.
    --skip <n>         Same as -s, --start.
    -e, --end <n>      The index of the row to slice to.
    -l, --len <n>      The length of the slice (can be used instead of --end).
    -i, --index <i>    Slice a single row (shortcut for -s N -l 1).
    -I, --indices <i>  Return a slice containing multiple indices at once.
                       You must provide the indices separated by commas,
                       e.g. \"1,4,67,89\". Note that selected rows will be
                       emitted in file order, not in the order given.
    -L, --last <n>     Return last <n> rows from file. Incompatible with other
                       flags. Runs in O(n) time & memory if file is seekable.
                       Else runs in O(N) time (N being the total number of rows of
                       the file) and O(n) memory.

slice options to use with expressions:
    -S, --start-condition <expr>  Do not start yielding rows until given expression
                                  returns true.
    -E, --end-condition <expr>    Stop yielding rows as soon as given expression
                                  returns false.

slice options to use with byte offets:
    -B, --byte-offset <b>  Byte offset to seek to in the sliced file. This can
                           be useful to access a particular slice of rows in
                           constant time, without needing to read preceding bytes.
                           You must provide a byte offset starting a CSV row or
                           the output could be corrupted. This requires the input
                           to be seekable (stdin or gzipped files not supported).
    --end-byte <b>         Only read up to provided position in byte, exclusive.
                           This requires the input to be seekable (stdin or gzipped
                           files not supported).
    --raw                  Raw slicing that forego parsing CSV data for better
                           performance. Only use if you know what you are doing.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Otherwise, the first row will always
                           appear in the output as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Default)]
pub struct Args {
    pub arg_input: Option<String>,
    flag_start: Option<usize>,
    flag_skip: Option<usize>,
    flag_end: Option<usize>,
    pub flag_len: Option<usize>,
    flag_index: Option<usize>,
    flag_indices: Option<String>,
    pub flag_last: Option<usize>,
    flag_start_condition: Option<String>,
    flag_end_condition: Option<String>,
    flag_byte_offset: Option<u64>,
    flag_end_byte: Option<u64>,
    flag_raw: bool,
    pub flag_output: Option<String>,
    pub flag_no_headers: bool,
    pub flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn resolve(&mut self) {
        if let (None, Some(skip)) = (self.flag_start, self.flag_skip) {
            self.flag_start = Some(skip);
        }
    }

    pub fn run(mut self) -> CliResult<()> {
        self.resolve();

        if self.flag_raw {
            if self.flag_byte_offset.is_none() || self.flag_end_byte.is_none() {
                Err("--raw requires both -B/--byte-offset & --end-byte!")?;
            }

            let rconf = self.rconfig();
            let wconf = self.wconfig();

            let mut wtr = wconf.io_writer()?;

            let mut rdr = rconf.io_reader_for_random_access()?;

            if !rconf.no_headers {
                let mut csv_rdr = rconf.csv_reader_from_reader(&mut rdr);
                let headers = csv_rdr.byte_headers()?;

                let mut csv_wtr = wconf.csv_writer_from_writer(&mut wtr);
                csv_wtr.write_byte_record(&headers)?;
            }

            let start = self.flag_byte_offset.unwrap();
            let end = self.flag_end_byte.unwrap();
            let limit = end.saturating_sub(start);

            rdr.seek(SeekFrom::Start(start))?;
            copy(&mut rdr.take(limit), &mut wtr)?;

            return Ok(());
        }

        if self.flag_last.is_some() {
            return self.run_last();
        }

        if self.flag_indices.is_some() {
            if self.flag_start_condition.is_some() || self.flag_end_condition.is_some() {
                Err(
                    "-I/--indices does not work with -S/--start-condition nor -E/--end-condition!",
                )?;
            }

            return {
                let rconf = self.rconfig();

                if let Some(offset) = self.flag_byte_offset {
                    let inner = rconf.io_reader_for_random_access()?;
                    let mut rdr = rconf.csv_reader_from_reader(inner);

                    let mut pos = csv::Position::new();
                    pos.set_byte(offset);

                    rdr.seek_raw(SeekFrom::Start(offset), pos)?;

                    self.run_plural(rdr)
                } else {
                    let rdr = rconf.reader()?;
                    self.run_plural(rdr)
                }
            };
        }

        let rconf = self.rconfig();

        if let Some(offset) = self.flag_byte_offset {
            let inner = rconf.io_reader_for_random_access()?;
            let mut rdr = rconf.csv_reader_from_reader(inner);

            let mut pos = csv::Position::new();
            pos.set_byte(offset);

            rdr.seek_raw(SeekFrom::Start(offset), pos)?;

            self.run_default(rdr)
        } else {
            let rdr = rconf.reader()?;
            self.run_default(rdr)
        }
    }

    fn run_default<R: Read>(&self, mut rdr: csv::Reader<R>) -> CliResult<()> {
        let mut wtr = self.wconfig().writer()?;
        self.rconfig().write_headers(&mut rdr, &mut wtr)?;

        let mut record = csv::ByteRecord::new();
        let mut conditions = self.conditions(rdr.byte_headers()?)?;

        let (start, end) = self.range()?;
        let mut record_index: usize = 0;
        let mut i: usize = 0;

        while read_byte_record_up_to(&mut rdr, &mut record, self.flag_end_byte)? {
            match conditions.process(record_index, &record)? {
                ControlFlow::Break => break,
                ControlFlow::Continue => continue,
                ControlFlow::Process => (),
            };

            i += 1;

            if i <= start {
                continue;
            }

            wtr.write_byte_record(&record)?;

            if i == end {
                break;
            }

            record_index += 1;
        }

        Ok(wtr.flush()?)
    }

    fn run_last(&self) -> CliResult<()> {
        let rconf = self.rconfig();
        let mut wtr = self.wconfig().writer()?;

        let n = self.flag_last.unwrap();

        match rconf.reverse_reader() {
            Ok((headers, mut reverse_reader)) => {
                if !self.flag_no_headers {
                    wtr.write_byte_record(&headers)?;
                }

                let records = reverse_reader
                    .byte_records()
                    .take(n)
                    .collect::<Result<Vec<_>, _>>()?;

                for record in records.into_iter().rev() {
                    wtr.write_record(
                        record
                            .iter()
                            .rev()
                            .map(|cell| cell.iter().rev().copied().collect::<Vec<_>>()),
                    )?;
                }
            }
            Err(_) => {
                let mut rdr = rconf.reader()?;

                let n = self.flag_last.unwrap();

                let headers = rdr.byte_headers()?.clone();

                if !self.flag_no_headers {
                    wtr.write_byte_record(&headers)?;
                }

                let mut buffer: VecDeque<csv::ByteRecord> = VecDeque::with_capacity(n);

                for result in rdr.byte_records() {
                    if buffer.len() >= n {
                        buffer.pop_front();
                    }

                    buffer.push_back(result?);
                }

                for record in buffer {
                    wtr.write_byte_record(&record)?;
                }
            }
        };

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

    fn conditions(&self, headers: &csv::ByteRecord) -> CliResult<Conditions> {
        let start_condition_program = self
            .flag_start_condition
            .as_ref()
            .map(|expr| Program::parse(expr, headers))
            .transpose()?;

        let end_condition_program = self
            .flag_end_condition
            .as_ref()
            .map(|expr| Program::parse(expr, headers))
            .transpose()?;

        Ok(Conditions {
            start: start_condition_program,
            end: end_condition_program,
            has_started: false,
        })
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    args.run()
}
