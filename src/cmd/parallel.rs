use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Parallel todo...

Usage:
    xan parallel (count|cat) [options] [<inputs>...]
    xan parallel --help

parallel options:
    -p, --preprocess <op>  Preprocessing command that will run on every
                           file to process.
    --progress             Display a progress bar for the parallel tasks.

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
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let total_count = AtomicUsize::new(0);

    args.arg_inputs
        .par_iter()
        .try_for_each(|name| -> CliResult<()> {
            let mut reader: csv::Reader<Box<dyn std::io::Read + Send>> =
                if let Some(_preprocessing) = &args.flag_preprocess {
                    let config = Config::new(&None)
                        .delimiter(args.flag_delimiter)
                        .no_headers(args.flag_no_headers);

                    let child = Command::new("cat")
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .arg(name)
                        .spawn()
                        .expect("could not spawn preprocessing");

                    config.csv_reader_from_reader(Box::new(
                        child.stdout.expect("cannot read child stdout"),
                    ))
                } else {
                    let config = Config::new(&Some(name.to_string()))
                        .delimiter(args.flag_delimiter)
                        .no_headers(args.flag_no_headers);

                    config.reader()?
                };

            let mut record = csv::ByteRecord::new();
            let mut count: usize = 0;

            while reader.read_byte_record(&mut record)? {
                count += 1;
            }

            total_count.fetch_add(count, Ordering::Relaxed);

            Ok(())
        })?;

    println!("{}", total_count.into_inner());

    Ok(())
}
