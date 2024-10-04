use std::num::NonZeroUsize;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

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

// TODO: cat without preprocessing is basically moot
// TODO: stding handling, csv stdin handling

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
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn reader(&self, path: &str) -> CliResult<csv::Reader<Box<dyn std::io::Read + Send>>> {
        Ok(if let Some(_preprocessing) = &self.flag_preprocess {
            let config = Config::new(&None)
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            let child = Command::new("cat")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .arg(path)
                .spawn()
                .expect("could not spawn preprocessing");

            config.csv_reader_from_reader(Box::new(child.stdout.expect("cannot read child stdout")))
        } else {
            let config = Config::new(&Some(path.to_string()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            config.reader()?
        })
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if let Some(threads) = args.flag_threads {
        ThreadPoolBuilder::new()
            .num_threads(threads.get())
            .build_global()
            .expect("could not build thread pool!");
    }

    let total_count = AtomicUsize::new(0);

    let total_inputs = args.arg_inputs.len();
    let progress_bar = if args.flag_progress {
        ParallelProgressBar::new(total_inputs)
    } else {
        ParallelProgressBar::hidden()
    };

    args.arg_inputs
        .par_iter()
        .try_for_each(|path| -> CliResult<()> {
            let mut reader = args.reader(path)?;
            let mut record = csv::ByteRecord::new();
            let mut count: usize = 0;

            while reader.read_byte_record(&mut record)? {
                count += 1;
            }

            total_count.fetch_add(count, Ordering::Relaxed);
            progress_bar.tick();

            Ok(())
        })?;

    progress_bar.abandon();

    println!("{}", total_count.into_inner());

    Ok(())
}
