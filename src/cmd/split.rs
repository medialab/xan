use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::Path;

use crate::config::{Config, Delimiter};
use crate::read::segment_csv_file;
use crate::util::{self, FilenameTemplate};
use crate::CliResult;

static USAGE: &str = "
Splits the given CSV data into chunks.

The files are written to the directory given with the name '{start}.csv',
where {start} is the index of the first record of the chunk (starting at 0).

Usage:
    xan split [options] <outdir> [<input>]
    xan split --help

split options:
    -s, --size <arg>       The number of records to write into each chunk.
                           [default: 4096]
    -c, --chunks <n>       Divide the file into approximately <n> chunks having
                           roughly the same number of records. Target file must be
                           seekable (e.g. this will not work with stdin nor gzipped
                           files).
    --segments             When used with -c/--chunks, output the byte offsets of
                           found segments insteads.
    --filename <filename>  A filename template to use when constructing
                           the names of the output files.  The string '{}'
                           will be replaced by a value based on the value
                           of the field, but sanitized for shell safety.
                           [default: {}.csv]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Otherwise, the first row will
                           appear in all chunks as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Clone, Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_outdir: String,
    flag_size: NonZeroUsize,
    flag_chunks: Option<NonZeroUsize>,
    flag_segments: bool,
    flag_filename: FilenameTemplate,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_chunks.is_some() {
        if args.flag_segments {
            args.segments()
        } else {
            unimplemented!()
        }
    } else {
        args.sequential_split()
    }
}

impl Args {
    fn sequential_split(&self) -> CliResult<()> {
        fs::create_dir_all(&self.arg_outdir)?;

        let rconfig = self.rconfig();
        let mut rdr = rconfig.reader()?;
        let headers = rdr.byte_headers()?.clone();

        let mut wtr = self.new_writer(&headers, 0)?;
        let mut i = 0;
        let mut row = csv::ByteRecord::new();
        while rdr.read_byte_record(&mut row)? {
            if i > 0 && i % self.flag_size == 0 {
                wtr.flush()?;
                wtr = self.new_writer(&headers, i)?;
            }
            wtr.write_byte_record(&row)?;
            i += 1;
        }
        wtr.flush()?;
        Ok(())
    }

    fn segments(&self) -> CliResult<()> {
        let rconfig = self.rconfig().flexible(true);
        let target = rconfig.io_reader_for_random_access()?;
        let mut reader = rconfig.csv_reader_from_reader(target);

        let offsets_opt = segment_csv_file(&mut reader, self.flag_chunks.unwrap().get(), 128, 8)?;

        match offsets_opt {
            None => Err("could not find segments!")?,
            Some(offsets) => {
                let mut wtr = Config::new(&None).writer()?;
                let mut record = csv::ByteRecord::new();

                record.push_field(b"from");
                record.push_field(b"to");

                wtr.write_byte_record(&record)?;

                for (f, t) in offsets {
                    record.clear();
                    record.push_field(f.to_string().as_bytes());
                    record.push_field(t.to_string().as_bytes());

                    wtr.write_byte_record(&record)?;
                }

                wtr.flush()?;
            }
        }

        Ok(())
    }

    fn new_writer(
        &self,
        headers: &csv::ByteRecord,
        start: usize,
    ) -> CliResult<csv::Writer<Box<dyn io::Write + Send + 'static>>> {
        let dir = Path::new(&self.arg_outdir);
        let path = dir.join(self.flag_filename.filename(&format!("{}", start)));
        let spath = Some(path.display().to_string());
        let mut wtr = Config::new(&spath).writer()?;
        if !self.rconfig().no_headers {
            wtr.write_record(headers)?;
        }
        Ok(wtr)
    }

    fn rconfig(&self) -> Config {
        Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
    }
}
