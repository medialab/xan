use std::collections::HashSet;
use std::num::NonZeroUsize;

use aho_corasick::AhoCorasick;
use bstr::ByteSlice;
use regex::bytes::{RegexBuilder, RegexSetBuilder};

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliError;
use crate::CliResult;

enum Matcher {
    Empty,
    NonEmpty,
    Substring(AhoCorasick, bool),
    Exact(Vec<u8>, bool),
    Regex(regex::bytes::Regex),
    ManyRegex(regex::bytes::RegexSet),
    ManyExact(HashSet<Vec<u8>>, bool),
}

impl Matcher {
    fn is_match(&self, cell: &[u8]) -> bool {
        match self {
            Self::Empty => cell.is_empty(),
            Self::NonEmpty => !cell.is_empty(),
            Self::Substring(pattern, case_insensitive) => {
                if *case_insensitive {
                    pattern.is_match(&cell.to_lowercase())
                } else {
                    pattern.is_match(cell)
                }
            }
            Self::Regex(pattern) => pattern.is_match(cell),
            Self::Exact(pattern, case_insensitive) => {
                if *case_insensitive {
                    &cell.to_lowercase() == pattern
                } else {
                    cell == pattern
                }
            }
            Self::ManyRegex(set) => set.is_match(cell),
            Self::ManyExact(patterns, case_insensitive) => {
                if *case_insensitive {
                    patterns.contains(&cell.to_lowercase())
                } else {
                    patterns.contains(cell)
                }
            }
        }
    }

    fn count(&self, cell: &[u8]) -> usize {
        match self {
            Self::Empty => {
                if cell.is_empty() {
                    1
                } else {
                    0
                }
            }
            Self::NonEmpty => {
                if cell.is_empty() {
                    0
                } else {
                    1
                }
            }
            Self::Substring(pattern, case_insensitive) => {
                if *case_insensitive {
                    pattern.find_iter(&cell.to_lowercase()).count()
                } else {
                    pattern.find_iter(cell).count()
                }
            }
            Self::Regex(pattern) => pattern.find_iter(cell).count(),
            Self::Exact(pattern, case_insensitive) => {
                if *case_insensitive {
                    if &cell.to_lowercase() == pattern {
                        1
                    } else {
                        0
                    }
                } else if cell == pattern {
                    1
                } else {
                    0
                }
            }
            Self::ManyRegex(set) => set.matches(cell).len(),
            Self::ManyExact(patterns, case_insensitive) => {
                if *case_insensitive {
                    if patterns.contains(&cell.to_lowercase()) {
                        1
                    } else {
                        0
                    }
                } else if patterns.contains(cell) {
                    1
                } else {
                    0
                }
            }
        }
    }
}

// NOTE: a -U, --unbuffered flag that flushes on each match does not solve
// early termination when piping to `xan slice` because flush won't get a broken
// pipe when writing nothing.
static USAGE: &str = "
Keep rows of given CSV file if ANY of the selected columns contains a desired
substring.

Can also be used to search for exact matches using the -e, --exact flag.

Can also be used to search using a regular expression using the -r, --regex flag.

Can also be used to search for empty or non-empty selections. For instance,
keeping only rows where selection is not fully empty:

    $ xan search --non-empty file.csv

Or keeping only rows where selection has any empty column:

    $ xan search --empty file.csv

When using a regular expression, be sure to mind bash escape rules (prefer single
quotes around your expression and don't forget to use backslashes when needed):

    $ xan search -r '\\bfran[cç]' file.csv

To restrict the columns that will be searched you can use the -s, --select flag.

All search modes can also be case-insensitive using -i, --ignore-case.

Finally, this command is also able to search for multiple patterns at once.
To do so, you must give a text file with one pattern per line to the --patterns
flag, or a CSV file containing a column of to indicate using --pattern-column.

One pattern per line of text file:

    $ xan search --patterns patterns.txt file.csv > matches.csv

CSV column containing patterns:

    $ xan search --patterns people.csv --pattern-column name tweets.csv > matches.csv

Feeding patterns through stdin (using \"-\"):

    $ cat patterns.txt | xan search --patterns - file.csv > matches.csv

Feeding CSV column as patterns through stdin (using \"-\"):

    $ xan slice -l 10 people.csv | xan search --patterns - --pattern-column name file.csv > matches.csv

Usage:
    xan search [options] --non-empty [<input>]
    xan search [options] --empty [<input>]
    xan search [options] --patterns <index> [<input>]
    xan search [options] <pattern> [<input>]
    xan search --help

search options:
    -e, --exact              Perform an exact match.
    -r, --regex              Use a regex to perform the match.
    -E, --empty              Search for empty cells, i.e. filter out
                             any completely non-empty selection.
    -N, --non-empty          Search for non-empty cells, i.e. filter out
                             any completely empty selection.
    --patterns <path>        Path to a text file (use \"-\" for stdin), containing multiple
                             patterns, one per line, to search at once.
    --pattern-column <name>  When given a column name, --patterns file will be considered a CSV
                             and patterns to search will be extracted from the given column.
    -i, --ignore-case        Case insensitive search.
    -s, --select <arg>       Select the columns to search. See 'xan select -h'
                             for the full syntax.
    -v, --invert-match       Select only rows that did not match
    -A, --all                Only return a row when ALL columns from the given selection
                             match the desired pattern, instead of returning a row
                             when ANY column matches.
    -c, --count <column>     If given, the command will not filter rows but will instead
                             count the total number of non-overlapping pattern matches per
                             row and report it in a new column with given name.
                             Does not work with -v/--invert-match.
    -l, --limit <n>          Maximum of number rows to return. Useful to avoid downstream
                             buffering some times (e.g. when searching for very few
                             rows in a big file before piping to `view` or `flatten`).

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_pattern: Option<String>,
    flag_select: SelectColumns,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_invert_match: bool,
    flag_all: bool,
    flag_ignore_case: bool,
    flag_empty: bool,
    flag_non_empty: bool,
    flag_exact: bool,
    flag_regex: bool,
    flag_count: Option<String>,
    flag_limit: Option<NonZeroUsize>,
    flag_patterns: Option<String>,
    flag_pattern_column: Option<SelectColumns>,
}

impl Args {
    fn build_matcher(&self) -> Result<Matcher, CliError> {
        if self.flag_non_empty {
            return Ok(Matcher::NonEmpty);
        }

        if self.flag_empty {
            return Ok(Matcher::Empty);
        }

        match self.flag_patterns.as_ref() {
            None => {
                let pattern = self.arg_pattern.as_ref().unwrap();

                Ok(if self.flag_exact {
                    if self.flag_ignore_case {
                        Matcher::Exact(pattern.as_bytes().to_lowercase(), true)
                    } else {
                        Matcher::Exact(pattern.as_bytes().to_vec(), false)
                    }
                } else if self.flag_regex {
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
            Some(_) => {
                let patterns = Config::new(&self.flag_patterns)
                    .delimiter(self.flag_delimiter)
                    .lines(&self.flag_pattern_column)?;

                Ok(if self.flag_exact {
                    Matcher::ManyExact(
                        patterns
                            .map(|pattern| {
                                pattern.map(|p| {
                                    if self.flag_ignore_case {
                                        p.to_lowercase().into_bytes()
                                    } else {
                                        p.into_bytes()
                                    }
                                })
                            })
                            .collect::<Result<HashSet<_>, _>>()?,
                        self.flag_ignore_case,
                    )
                } else if self.flag_regex {
                    Matcher::ManyRegex(
                        RegexSetBuilder::new(&patterns.collect::<Result<Vec<_>, _>>()?)
                            .case_insensitive(self.flag_ignore_case)
                            .build()?,
                    )
                } else {
                    Matcher::Substring(
                        AhoCorasick::new(
                            &patterns
                                .map(|pattern| {
                                    pattern.map(|p| {
                                        if self.flag_ignore_case {
                                            p.to_lowercase()
                                        } else {
                                            p
                                        }
                                    })
                                })
                                .collect::<Result<Vec<_>, _>>()?,
                        )?,
                        self.flag_ignore_case,
                    )
                })
            }
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let matchers_count: u8 = args.flag_exact as u8
        + args.flag_regex as u8
        + args.flag_non_empty as u8
        + args.flag_empty as u8;

    if matchers_count > 1 {
        Err("must select only one of -e/--exact, -N/--non-empty, -E/--empty or -r/--regex!")?;
    }

    if args.flag_count.is_some() && args.flag_invert_match {
        Err("-c/--count does not work with -v/--invert-match!")?;
    }

    let matcher = args.build_matcher()?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    if let Some(column_name) = &args.flag_count {
        headers.push_field(column_name.as_bytes());
    }

    if !rconfig.no_headers {
        wtr.write_record(&headers)?;
    }

    let mut record = csv::ByteRecord::new();
    let mut i: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        let mut is_match: bool = false;

        if args.flag_count.is_some() {
            let count: usize = sel.select(&record).map(|cell| matcher.count(cell)).sum();

            if count > 0 {
                is_match = true;
            }

            record.push_field(count.to_string().as_bytes());
            wtr.write_byte_record(&record)?;
        } else {
            is_match = if args.flag_all {
                sel.select(&record).all(|cell| matcher.is_match(cell))
            } else {
                sel.select(&record).any(|cell| matcher.is_match(cell))
            };

            if args.flag_invert_match {
                is_match = !is_match;
            }

            if is_match {
                wtr.write_byte_record(&record)?;
            }
        }

        if let Some(limit) = args.flag_limit {
            if is_match {
                i += 1;
            }

            if i >= limit.get() {
                break;
            }
        }
    }

    Ok(wtr.flush()?)
}
