use std::collections::VecDeque;
use std::io::Write;
use std::num::NonZeroUsize;

use aho_corasick::AhoCorasick;
use regex::bytes::RegexBuilder;
use regex_automata::{meta::Regex as RegexSet, util::syntax};

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliError;
use crate::CliResult;

use crate::cmd::search::Matcher;

struct BeforeContextBuffer {
    buffer: VecDeque<Vec<u8>>,
}

impl BeforeContextBuffer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
        }
    }

    #[inline]
    fn push(&mut self, record: &[u8]) {
        if self.buffer.len() == self.buffer.capacity() {
            self.buffer.pop_front();
        }

        self.buffer.push_back(record.to_vec());
    }

    #[inline(always)]
    fn flush(&mut self) -> impl Iterator<Item = Vec<u8>> + '_ {
        self.buffer.drain(..)
    }
}

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
    xan grep [options] --patterns <path> [<input>]
    xan grep [options] <pattern> [-P <pattern>...] [<input>]
    xan grep --help

grep options:
    -c, --count               Only return the number of matching rows.
    -r, --regex               Matches the given pattern as a regex.
    -i, --ignore-case         Ignore case while matching rows.
    -v, --invert-match        Only return or count rows that did not match
                              given pattern.
    -B, --before-context <n>  Number of rows to keep before a matching one.
    -A, --after-context <n>   Number of rows to keep after a matching one.
    --mmap                    Use a memory map to speed up computations. Only
                              works if the file is on disk (no streams) and if the
                              file is uncompressed. Usually a bad idea on macOS.

multiple patterns options:
    -P, --add-pattern <pattern>  Manually add patterns to query without needing to feed a file
                                 to the --patterns flag.
    --patterns <path>            Path to a text file (use \"-\" for stdin), containing multiple
                                 patterns, one per line, to search at once.
    --pattern-column <name>      When given a column name, --patterns file will be considered a CSV
                                 and patterns to search will be extracted from the given column.

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
    arg_pattern: Option<String>,
    arg_input: Option<String>,
    flag_count: bool,
    flag_regex: bool,
    flag_ignore_case: bool,
    flag_invert_match: bool,
    flag_before_context: Option<NonZeroUsize>,
    flag_after_context: Option<NonZeroUsize>,
    flag_mmap: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_patterns: Option<String>,
    flag_pattern_column: Option<SelectedColumns>,
    flag_add_pattern: Vec<String>,
}

impl Args {
    fn build_matcher(&self, patterns: &Option<Vec<String>>) -> Result<Matcher, CliError> {
        match patterns {
            None => {
                let pattern = self.arg_pattern.as_ref().unwrap();

                Ok(if self.flag_regex {
                    Matcher::Regex(
                        RegexBuilder::new(pattern)
                            .case_insensitive(self.flag_ignore_case)
                            .build()?,
                    )
                } else {
                    Matcher::Substring(
                        AhoCorasick::new([if self.flag_ignore_case {
                            pattern.to_lowercase()
                        } else {
                            pattern.to_string()
                        }])?,
                        self.flag_ignore_case,
                    )
                })
            }
            Some(patterns) => Ok(if self.flag_regex {
                Matcher::RegexSet(
                    RegexSet::builder()
                        .syntax(syntax::Config::new().case_insensitive(self.flag_ignore_case))
                        .build_many(&patterns.iter().collect::<Vec<_>>())?,
                )
            } else {
                Matcher::Substring(
                    AhoCorasick::new(
                        patterns
                            .iter()
                            .map(|pattern| {
                                if self.flag_ignore_case {
                                    pattern.to_lowercase()
                                } else {
                                    pattern.to_string()
                                }
                            })
                            .collect::<Vec<_>>(),
                    )?,
                    self.flag_ignore_case,
                )
            }),
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if (args.flag_before_context.is_some() || args.flag_after_context.is_some()) && args.flag_count
    {
        Err("-c/--count does not work with -B/--before-context nor -A/--after-context!")?;
    }

    if !args.flag_add_pattern.is_empty() && args.flag_patterns.is_some() {
        Err("-P/--add-pattern is incompatible with --patterns!")?;
    }

    let patterns = if !args.flag_add_pattern.is_empty() {
        let mut patterns = vec![args.arg_pattern.clone().unwrap()];

        for pattern in args.flag_add_pattern.iter() {
            patterns.push(pattern.to_string());
        }
        Some(patterns)
    } else if args.flag_patterns.is_some() {
        let mut patterns: Vec<String> = vec![];

        for result in Config::new(&Some(args.flag_patterns.clone().unwrap()))
            .lines(&args.flag_pattern_column)?
        {
            let pattern = result?;
            patterns.push(pattern);
        }
        Some(patterns)
    } else {
        None
    };
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let wconf = Config::new(&args.flag_output);

    let mut writer_opt = (!args.flag_count)
        .then(|| wconf.simd_writer())
        .transpose()?;

    let matcher = args.build_matcher(&patterns)?;

    let mut count: u64 = 0;
    let mut after_context_count: usize = 0;

    let mut before_context_buffer_opt = args
        .flag_before_context
        .map(|n| BeforeContextBuffer::with_capacity(n.get()));

    macro_rules! process_record {
        ($record: expr) => {
            let mut is_match = matcher.is_match($record)?;

            if args.flag_invert_match {
                is_match = !is_match;
            }

            if is_match {
                if let Some(writer) = writer_opt.as_mut() {
                    if let Some(buffer) = before_context_buffer_opt.as_mut() {
                        for past_record in buffer.flush() {
                            writer.write_splitted_record(&past_record)?;
                        }
                    }

                    writer.write_splitted_record($record)?;
                } else {
                    count += 1;
                }

                if let Some(n) = args.flag_after_context {
                    after_context_count = n.get();
                }
            } else {
                if after_context_count > 0 {
                    after_context_count -= 1;
                    writer_opt
                        .as_mut()
                        .unwrap()
                        .write_splitted_record($record)?;
                } else {
                    if let Some(buffer) = before_context_buffer_opt.as_mut() {
                        buffer.push($record);
                    }
                }
            }
        };
    }

    if args.flag_mmap {
        let map = rconf.mmap()?.ok_or("Cannot use --mmap on target!")?;

        let mut reader = simd_csv::TotalReaderBuilder::new()
            .delimiter(rconf.delimiter)
            .has_headers(false)
            .from_bytes(&map);

        if !rconf.no_headers {
            if let Some(header) = reader.split_record() {
                if let Some(writer) = writer_opt.as_mut() {
                    writer.write_splitted_record(header)?;
                }
            }
        }

        while let Some(record) = reader.split_record() {
            process_record!(record);
        }
    } else {
        let mut splitter = rconf.simd_splitter()?;

        if !rconf.no_headers {
            if let Some(writer) = writer_opt.as_mut() {
                writer.write_splitted_record(splitter.byte_headers()?)?;
            }
        }

        while let Some(record) = splitter.split_record()? {
            process_record!(record);
        }
    }

    if let Some(writer) = writer_opt.as_mut() {
        writer.flush()?;
    } else {
        writeln!(wconf.io_writer()?, "{}", count)?;
    }

    Ok(())
}
