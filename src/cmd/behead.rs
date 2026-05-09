use std::fs::OpenOptions;
use std::io::{self, Write};

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Drop a CSV file's header.

Note that to be as performant as possible, this command does not try
to be clever and only parses the first CSV row to drop it. The rest of
the file will be flushed to the output as-is without any kind of normalization.

Usage:
    xan behead [options] [<input>]
    xan guillotine [options] [<input>]

behead options:
    -A, --append  Only drop headers if output already exists and
                  is not empty. Requires -o/--output to be set.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_append: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_append && args.flag_output.is_none() {
        Err("-A/--append needs to know where the output will be written!\nPlease provide -o/--output.")?;
    }

    let wconf = Config::new(&args.flag_output);

    let mut actually_behead = true;

    if args.flag_append {
        let output_path = wconf.path.as_ref().unwrap();

        if !output_path.is_file() || output_path.metadata()?.len() == 0 {
            actually_behead = false;
        }
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(!actually_behead);

    let mut peeker = rconf.simd_peeker()?;
    peeker.peek_byte_headers()?;

    let mut wtr = wconf.buf_io_writer_with_options(
        OpenOptions::new()
            .write(true)
            .create(true)
            .append(args.flag_append),
    )?;

    io::copy(&mut peeker.into_reader(), &mut wtr)?;

    Ok(wtr.flush()?)
}
