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

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(true);

    let mut splitter = rconf.simd_splitter()?;
    let header_opt = splitter.split_record()?;

    let wconf = Config::new(&args.flag_output);

    let mut wtr = wconf.buf_io_writer_with_options(
        OpenOptions::new()
            .write(true)
            .create(true)
            .append(args.flag_append),
    )?;

    if args.flag_append && wconf.path.unwrap().metadata()?.len() == 0 {
        if let Some(header) = header_opt {
            wtr.write_all(header)?;
            wtr.write_all(b"\n")?;
        }
    }

    io::copy(&mut splitter.into_bufreader().1, &mut wtr)?;

    Ok(wtr.flush()?)
}
