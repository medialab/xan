use std::collections::HashSet;

use aho_corasick::AhoCorasick;
use csv;
use regex::bytes::{RegexBuilder, RegexSetBuilder};

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliError;
use crate::CliResult;

fn lowercase(cell: &[u8]) -> String {
    std::str::from_utf8(cell).unwrap().to_lowercase()
}

enum Matcher {
    NonEmpty,
    Substring(AhoCorasick, bool),
    Exact(Vec<u8>),
    ExactCaseInsensitive(String),
    Regex(regex::bytes::Regex),
    ManyRegex(regex::bytes::RegexSet),
    ManyExact(HashSet<Vec<u8>>),
    ManyExactCaseInsensitive(HashSet<String>),
}

impl Matcher {
    fn is_match(&self, cell: &[u8]) -> bool {
        match self {
            Self::NonEmpty => !cell.is_empty(),
            Self::Substring(pattern, case_insensitive) => {
                if *case_insensitive {
                    pattern.is_match(&lowercase(cell))
                } else {
                    pattern.is_match(cell)
                }
            }
            Self::Regex(pattern) => pattern.is_match(cell),
            Self::Exact(pattern) => pattern == cell,
            Self::ExactCaseInsensitive(pattern) => &lowercase(cell) == pattern,
            Self::ManyRegex(set) => set.is_match(cell),
            Self::ManyExact(patterns) => patterns.contains(cell),
            Self::ManyExactCaseInsensitive(patterns) => patterns.contains(&lowercase(cell)),
        }
    }
}

static USAGE: &str = "
Filter rows of given CSV file if some of its cells contains a desired substring.

Can also be used to search for exact matches using the -e, --exact flag.

Can also be used to search using a regular expression using the -r, --regex flag.

When using a regular expression, be sure to mind bash escape rules (prefer single
quotes around your expression and don't forget to use backslashes when needed):

    $ xan search -r '\\bfran[cç]' file.csv

To restrict the columns that will be searched you can use the -s, --select flag.

All search modes can also be case-insensitive using -i, --ignore-case.

Finally, this command is also able to take a CSV file column containing multiple
patterns to search for at once, using the --input flag:

    $ xan search --input user-ids.csv user_id tweets.csv

Usage:
    xan search [options] --non-empty [<input>]
    xan search [options] --input <index> <column> [<input>]
    xan search [options] <pattern> [<input>]
    xan search --help

search options:
    -e, --exact            Perform an exact match.
    -r, --regex            Use a regex to perform the match.
    -N, --non-empty        Search for non-empty cells, i.e. filter out
                           any completely empty selection.
    --input <index>        CSV file containing a column of value to index & search.
    -i, --ignore-case      Case insensitive search. This is equivalent to
                           prefixing the regex with '(?i)'.
    -s, --select <arg>     Select the columns to search. See 'xan select -h'
                           for the full syntax.
    -v, --invert-match     Select only rows that did not match
    -f, --flag <column>    If given, the command will not filter rows
                           but will instead flag the found rows in a new
                           column with given name.

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
    arg_column: Option<SelectColumns>,
    flag_select: SelectColumns,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_invert_match: bool,
    flag_ignore_case: bool,
    flag_non_empty: bool,
    flag_exact: bool,
    flag_regex: bool,
    flag_flag: Option<String>,
    flag_input: Option<String>,
}

impl Args {
    fn get_matcher(&self) -> Result<Matcher, CliError> {
        if self.flag_non_empty {
            return Ok(Matcher::NonEmpty);
        }

        match self.arg_column.as_ref() {
            None => {
                let pattern = self.arg_pattern.as_ref().unwrap();

                Ok(if self.flag_exact {
                    if self.flag_ignore_case {
                        Matcher::ExactCaseInsensitive(pattern.to_lowercase())
                    } else {
                        Matcher::Exact(pattern.as_bytes().to_vec())
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
            Some(column) => {
                let rconf = Config::new(&self.flag_input)
                    .delimiter(self.flag_delimiter)
                    .select(column.clone());

                let mut rdr = rconf.reader()?;

                let headers = rdr.byte_headers()?;
                let column_index = rconf.single_selection(headers)?;

                let mut record = csv::ByteRecord::new();

                let mut set: HashSet<Vec<u8>> = HashSet::new();
                let mut lower_set: HashSet<String> = HashSet::new();
                let mut patterns: Vec<String> = Vec::new();

                while rdr.read_byte_record(&mut record)? {
                    let pattern = &record[column_index];

                    if self.flag_exact {
                        if self.flag_ignore_case {
                            lower_set.insert(lowercase(pattern));
                        } else {
                            set.insert(pattern.to_vec());
                        }
                    } else {
                        patterns.push(std::str::from_utf8(pattern).unwrap().to_string());
                    }
                }

                Ok(if self.flag_exact {
                    if self.flag_ignore_case {
                        Matcher::ManyExactCaseInsensitive(lower_set)
                    } else {
                        Matcher::ManyExact(set)
                    }
                } else if self.flag_regex {
                    Matcher::ManyRegex(
                        RegexSetBuilder::new(&patterns)
                            .case_insensitive(self.flag_ignore_case)
                            .build()?,
                    )
                } else {
                    Matcher::Substring(AhoCorasick::new(&patterns)?, self.flag_ignore_case)
                })
            }
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let mut matchers_count: u8 = 0;

    if args.flag_exact {
        matchers_count += 1;
    }
    if args.flag_regex {
        matchers_count += 1;
    }
    if args.flag_non_empty {
        matchers_count += 1;
    }

    if matchers_count > 1 {
        Err("must select only one of -e/--exact, -N,--non-empty, -r,--regex!")?;
    }

    let matcher = args.get_matcher()?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    if let Some(column_name) = args.flag_flag.clone() {
        headers.push_field(column_name.as_bytes());
    }

    if !rconfig.no_headers {
        wtr.write_record(&headers)?;
    }

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        let mut is_match = sel.select(&record).any(|cell| matcher.is_match(cell));

        if args.flag_invert_match {
            is_match = !is_match;
        }

        if args.flag_flag.is_some() {
            record.push_field(if is_match { b"1" } else { b"0" });
            wtr.write_byte_record(&record)?;
        } else if is_match {
            wtr.write_byte_record(&record)?;
        }
    }
    Ok(wtr.flush()?)
}
