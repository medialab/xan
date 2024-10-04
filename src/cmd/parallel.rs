use std::num::NonZeroUsize;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use indicatif::ProgressBar;
use rayon::{prelude::*, ThreadPoolBuilder};

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

struct ParallelProgressBar {
    bar: Option<ProgressBar>,
}

impl ParallelProgressBar {
    fn hidden() -> Self {
        Self { bar: None }
    }

    fn new(total: usize) -> Self {
        Self {
            bar: Some(ProgressBar::new(total as u64)),
        }
    }

    fn abandon(&self) {
        if let Some(bar) = &self.bar {
            bar.abandon();
        }
    }

    fn tick(&self) {
        if let Some(bar) = &self.bar {
            bar.inc(1);
        }
    }
}

struct Children {
    children: Vec<Child>,
}

impl Children {
    fn pair(one: Child, two: Child) -> Self {
        Self {
            children: vec![one, two],
        }
    }

    fn wait(&mut self) -> std::io::Result<()> {
        for child in self.children.iter_mut() {
            child.wait()?;
        }

        Ok(())
    }
}

// TODO: cat without preprocessing is basically moot
// TODO: stdin handling, csv stdin handling
// TODO: cat -S/--source-column

static USAGE: &str = "
Count, filter & aggregate CSV datasets split into multiple
files, in parallel.

Usage:
    xan parallel count [options] [<inputs>...]
    xan parallel cat [options] [<inputs>...]
    xan parallel --help

parallel options:
    -p, --preprocess <op>  Preprocessing command that will run on every
                           file to process.
    --progress             Display a progress bar for the parallel tasks.
    -t, --threads <n>      Number of threads to use. Will default to a sensible
                           number based on the available CPUs.

parallel cat options:
    -B, --buffer-size <n>  Number of rows a thread is allowed to keep in memory
                           before flushing to the output.
                           [default: 1024]

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
    arg_inputs: Vec<String>,
    cmd_count: bool,
    cmd_cat: bool,
    flag_preprocess: Option<String>,
    flag_progress: bool,
    flag_threads: Option<NonZeroUsize>,
    flag_buffer_size: NonZeroUsize,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

type Reader = csv::Reader<Box<dyn std::io::Read + Send>>;

impl Args {
    fn reader(&self, path: &str) -> CliResult<(Reader, Option<Children>)> {
        Ok(if let Some(preprocessing) = &self.flag_preprocess {
            let config = Config::new(&None)
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let shell = std::env::var("SHELL").expect("$SHELL is not set!");

            let mut cat = Command::new("cat")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .arg(path)
                .spawn()
                .expect("could not spawn \"cat\"");

            let mut child = Command::new(shell)
                .stdin(cat.stdout.take().expect("could not consume cat stdout"))
                .stdout(Stdio::piped())
                .args(["-c", preprocessing])
                .spawn()
                .expect("could not spawn preprocessing");

            (
                config.csv_reader_from_reader(Box::new(
                    child.stdout.take().expect("cannot read child stdout"),
                )),
                Some(Children::pair(cat, child)),
            )
        } else {
            let config = Config::new(&Some(path.to_string()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            (config.reader()?, None)
        })
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_cat && args.flag_preprocess.is_none() {
        Err("`xan parallel cat` without -p/--preprocess is counterproductive!\n`xan cat rows` will be faster.")?
    }

    if let Some(threads) = args.flag_threads {
        ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .build_global()
            .expect("could not build thread pool!");
    }

    let inputs_count = args.arg_inputs.len();
    let progress_bar = if args.flag_progress {
        ParallelProgressBar::new(inputs_count)
    } else {
        ParallelProgressBar::hidden()
    };

    // Count
    if args.cmd_count {
        let total_count = AtomicUsize::new(0);

        args.arg_inputs
            .par_iter()
            .try_for_each(|path| -> CliResult<()> {
                let (mut reader, mut children_opt) = args.reader(path)?;

                let mut record = csv::ByteRecord::new();
                let mut count: usize = 0;

                while reader.read_byte_record(&mut record)? {
                    count += 1;
                }

                total_count.fetch_add(count, Ordering::Relaxed);
                progress_bar.tick();

                if let Some(children) = children_opt.as_mut() {
                    children.wait()?;
                }

                Ok(())
            })?;

        progress_bar.abandon();

        println!("{}", total_count.into_inner());
    }
    // Cat
    else if args.cmd_cat {
        let writer_mutex = Arc::new(Mutex::new((
            false,
            Config::new(&args.flag_output).writer()?,
        )));
        let buffer_size = args.flag_buffer_size.get();

        let flush = |headers: &csv::ByteRecord, records: &[csv::ByteRecord]| -> CliResult<()> {
            let mut guard = writer_mutex.lock().unwrap();

            if !guard.0 {
                guard.1.write_byte_record(headers)?;
                guard.0 = true;
            }

            for record in records.iter() {
                guard.1.write_byte_record(record)?;
            }

            Ok(())
        };

        args.arg_inputs
            .par_iter()
            .try_for_each(|path| -> CliResult<()> {
                let (mut reader, mut children_opt) = args.reader(path)?;
                let headers = reader.byte_headers()?.clone();

                let mut buffer: Vec<csv::ByteRecord> = Vec::with_capacity(buffer_size);

                for result in reader.byte_records() {
                    if buffer.len() == buffer_size {
                        flush(&headers, &buffer)?;

                        buffer.clear();
                    }

                    buffer.push(result?);
                }

                if !buffer.is_empty() {
                    flush(&headers, &buffer)?;
                }

                progress_bar.tick();

                if let Some(children) = children_opt.as_mut() {
                    children.wait()?;
                }

                Ok(())
            })?;

        progress_bar.abandon();

        writer_mutex.lock().unwrap().1.flush()?;
    }

    Ok(())
}
