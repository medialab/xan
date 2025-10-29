use std::io::Write;

use aho_corasick::AhoCorasick;
use regex::bytes::RegexBuilder;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

use crate::cmd::search::Matcher;

static USAGE: &str = "
Keep rows of a CSV file matching a given pattern. It can be thought of as
a CSV-aware version of the well-known `grep` command.

This command is faster than `xan search` because it relies on an optimized CSV
parser that only knows how to separate rows and does not care about finding cell
delimitations. But this also means this command has less features and is less
precise than `xan search` because it will try to match the given pattern on whole
rows at once, quotes & delimiters included. This is usually not an issue for coarse
filtering, but keep in mind it could be problematic for your use case.

Note also that if your CSV data has no quoting whatsoever, you really should
use `ripgrep` instead:
https://github.com/BurntSushi/ripgrep

Finally, contrary to most `xan` commands that will normalize the output to
standardish CSV data with commas and quoting using double quotes, this command
will output rows as-is, without any transformation.

Usage:
    xan grep [options] <pattern> [<input>]
    xan grep --help

grep options:
    -c, --count         Only return the number of matching rows.
    -r, --regex         Matches the given pattern as a regex.
    -i, --ignore-case   Ignore case while matching rows.
    -v, --invert-match  Only return or count rows that did not match
                        given pattern.
    --mmap              Use a memory map to speed up computations. Only
                        works if the file is on disk (no streams) and if the
                        file is uncompressed. Usually a bad idea on macOS.

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
    flag_regex: bool,
    flag_ignore_case: bool,
    flag_invert_match: bool,
    flag_mmap: bool,
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

    let matcher = if args.flag_regex {
        Matcher::Regex(
            RegexBuilder::new(&args.arg_pattern)
                .case_insensitive(args.flag_ignore_case)
                .build()?,
        )
    } else {
        let pattern = if args.flag_ignore_case {
            args.arg_pattern.to_lowercase()
        } else {
            args.arg_pattern.clone()
        };

        Matcher::Substring(AhoCorasick::new([&pattern])?, args.flag_ignore_case)
    };

    let mut count: u64 = 0;

    if args.flag_mmap {
        let map = rconf.mmap()?.ok_or("Cannot use --mmap on target!")?;

        let mut reader = simd_csv::TotalReaderBuilder::new()
            .delimiter(rconf.delimiter)
            .has_headers(false)
            .from_bytes(&map);

        if !rconf.no_headers {
            if let Some(header) = reader.split_record() {
                if let Some(writer) = writer_opt.as_mut() {
                    writer.write_all(header)?;
                    writer.write_all(b"\n")?;
                }
            }
        }

        while let Some(record) = reader.split_record() {
            let mut is_match = matcher.is_match(record);

            if args.flag_invert_match {
                is_match = !is_match;
            }

            if !is_match {
                continue;
            }

            if let Some(writer) = writer_opt.as_mut() {
                writer.write_all(record)?;
                writer.write_all(b"\n")?;
            } else {
                count += 1;
            }
        }
    } else {
        let mut splitter = rconf.simd_splitter()?;

        if !rconf.no_headers {
            if let Some(header) = splitter.split_record()? {
                if let Some(writer) = writer_opt.as_mut() {
                    writer.write_all(header)?;
                    writer.write_all(b"\n")?;
                }
            }
        }

        while let Some(record) = splitter.split_record()? {
            let mut is_match = matcher.is_match(record);

            if args.flag_invert_match {
                is_match = !is_match;
            }

            if !is_match {
                continue;
            }

            if let Some(writer) = writer_opt.as_mut() {
                writer.write_all(record)?;
                writer.write_all(b"\n")?;
            } else {
                count += 1;
            }
        }
    }

    if let Some(writer) = writer_opt.as_mut() {
        writer.flush()?;
    } else {
        writeln!(wconf.io_writer()?, "{}", count)?;
    }

    Ok(())
}
