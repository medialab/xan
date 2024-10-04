use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use rayon::prelude::*;

use crate::config::Delimiter;
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
            let preprocessing = Command::new("cat")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .arg(name)
                .spawn()
                .expect("could not spawn preprocessing");

            let mut reader_builder = csv::ReaderBuilder::new();
            reader_builder.flexible(false);
            reader_builder.has_headers(!args.flag_no_headers);

            if let Some(delimiter) = args.flag_delimiter {
                reader_builder.delimiter(delimiter.as_byte());
            }

            let mut reader =
                reader_builder.from_reader(preprocessing.stdout.expect("cannot read child stdout"));

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
