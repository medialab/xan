use std::io::Write;

use aho_corasick::AhoCorasick;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

use crate::cmd::search::Matcher;

static USAGE: &str = "
TODO...

Usage:
    xan grep [options] <pattern> [<input>]

grep options:
    -c, --count  If set, only returns the number of matching records.

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
    arg_pattern: String,
    arg_input: Option<String>,
    flag_count: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);
    let wconf = Config::new(&args.flag_output);

    let mut writer_opt = (!args.flag_count)
        .then(|| wconf.buf_io_writer())
        .transpose()?;

    let matcher = Matcher::Substring(AhoCorasick::new([&args.arg_pattern])?, false);

    let mut splitter = rconf.simd_splitter()?;
    let mut count: u64 = 0;

    if !args.flag_no_headers {
        if let Some(header) = splitter.split_record()? {
            if let Some(writer) = writer_opt.as_mut() {
                writer.write_all(header)?;
                writer.write_all(b"\n")?;
            }
        }
    }

    while let Some(record) = splitter.split_record()? {
        if !matcher.is_match(record) {
            continue;
        }

        if let Some(writer) = writer_opt.as_mut() {
            writer.write_all(record)?;
            writer.write_all(b"\n")?;
        } else {
            count += 1;
        }
    }

    if let Some(writer) = writer_opt.as_mut() {
        writer.flush()?;
    } else {
        writeln!(wconf.io_writer()?, "{}", count)?;
    }

    Ok(())
}
