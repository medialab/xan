use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::Path;

use crate::config::{Config, Delimiter};
use crate::read::{segment_csv_file, SegmentationOptions};
use crate::util::{self, FilenameTemplate};
use crate::CliResult;

static USAGE: &str = "
Splits the given CSV data into smaller files having a fixed number of
rows given to -s, --size.

Target file can also be split into a given number of -c/--chunks.

Files will be written in current working directory by default or in any directory
given to -O/--out-dir (that will be created for your if necessary).

Usage:
    xan split [options] [<input>]
    xan split --help

split options:
    -O, --out-dir <dir>        Where to write the chunks. Defaults to current working
                               directory.
    -S, --size <arg>           The number of records to write into each chunk.
                               [default: 4096]
    -c, --chunks <n>           Divide the file into at most <n> chunks having
                               roughly the same number of records. Target file must be
                               seekable (e.g. this will not work with stdin nor gzipped
                               files).
    --segments                 When used with -c/--chunks, output the byte offsets of
                               found segments insteads.
    -f, --filename <filename>  A filename template to use when constructing
                               the names of the output files. The string '{}'
                               will be replaced either by the index in original file of
                               first row emitted when using -S/--size or by the chunk
                               index when using -c/--chunks.
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
    flag_out_dir: Option<String>,
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
            args.split_by_segments()
        }
    } else {
        args.split_by_size()
    }
}

impl Args {
    fn split_by_size(&self) -> CliResult<()> {
        if let Some(out_dir) = &self.flag_out_dir {
            fs::create_dir_all(out_dir)?;
        }

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

        Ok(wtr.flush()?)
    }

    fn split_by_segments(&self) -> CliResult<()> {
        if let Some(out_dir) = &self.flag_out_dir {
            fs::create_dir_all(out_dir)?;
        }

        let rconfig = self.rconfig();
        let mut reader = rconfig.seekable_reader()?;

        let segments = segment_csv_file(
            &mut reader,
            SegmentationOptions::chunks(self.flag_chunks.unwrap().get()),
        )?
        .ok_or("could not segment the file properly!")?;

        let mut reader = rconfig.reader()?;
        let headers = reader.byte_headers()?.clone();

        let mut record = csv::ByteRecord::new();
        let mut writer = self.new_writer(&headers, 0)?;
        let mut chunk: usize = 0;

        while reader.read_byte_record(&mut record)? {
            if record.position().unwrap().byte() >= segments[chunk].1 {
                writer.flush()?;
                chunk += 1;
                writer = self.new_writer(&headers, chunk)?;
            }

            writer.write_byte_record(&record)?;
        }

        Ok(())
    }

    fn segments(&self) -> CliResult<()> {
        let rconfig = self.rconfig();
        let mut reader = rconfig.seekable_reader()?;

        let segments = segment_csv_file(
            &mut reader,
            SegmentationOptions::chunks(self.flag_chunks.unwrap().get()),
        )?
        .ok_or("could not segment the file properly!")?;

        let mut wtr = Config::new(&None).writer()?;
        let mut record = csv::ByteRecord::new();

        record.push_field(b"from");
        record.push_field(b"to");

        wtr.write_byte_record(&record)?;

        for (f, t) in segments {
            record.clear();
            record.push_field(f.to_string().as_bytes());
            record.push_field(t.to_string().as_bytes());

            wtr.write_byte_record(&record)?;
        }

        wtr.flush()?;

        Ok(())
    }

    fn new_writer(
        &self,
        headers: &csv::ByteRecord,
        id: usize,
    ) -> CliResult<csv::Writer<Box<dyn io::Write + Send + 'static>>> {
        let dir = match &self.flag_out_dir {
            Some(out_dir) => Path::new(out_dir),
            None => Path::new(""),
        };
        let path = dir.join(self.flag_filename.filename(&format!("{}", id)));
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
