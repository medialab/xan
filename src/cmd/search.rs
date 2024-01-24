use std::collections::HashSet;

use csv;
use regex::bytes::RegexBuilder;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliError;
use CliResult;

fn lowercase(cell: &[u8]) -> String {
    std::str::from_utf8(cell).unwrap().to_lowercase()
}

enum Matcher {
    Regex(regex::bytes::Regex),
    Exact(Vec<u8>),
    ExactCaseInsensitive(String),
    ManyExact(HashSet<Vec<u8>>),
    ManyExactCaseInsensitive(HashSet<String>),
}

impl Matcher {
    fn is_match(&self, cell: &[u8]) -> bool {
        match self {
            Self::Regex(pattern) => pattern.is_match(cell),
            Self::Exact(pattern) => pattern == cell,
            Self::ExactCaseInsensitive(pattern) => &lowercase(cell) == pattern,
            Self::ManyExact(patterns) => patterns.contains(cell),
            Self::ManyExactCaseInsensitive(patterns) => patterns.contains(&lowercase(cell)),
        }
    }
}

static USAGE: &str = "
Filters CSV data by whether the given regex matches a row.

The regex is applied to each field in each row, and if any field matches,
then the row is written to the output. The columns to search can be limited
with the '--select' flag (but the full row is still written to the output if
there is a match).

When giving a regex, be sure to mind bash escape rules (prefer single quotes
around your expression and don't forget to use backslash when needed).

Usage:
    xsv search [options] <column> --input <index> [<input>]
    xsv search [options] <pattern> [<input>]
    xsv search --help

search options:
    -e, --exact            Perform an exact match rather than using a
                           regular expression.
    --input <index>        CSV file containing a column of value to index & search.
    -i, --ignore-case      Case insensitive search. This is equivalent to
                           prefixing the regex with '(?i)'.
    -s, --select <arg>     Select the columns to search. See 'xsv select -h'
                           for the full syntax.
    -v, --invert-match     Select only rows that did not match

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
    -f, --flag <column>    If given, the command will not filter rows
                           but will instead flag the found rows in a new
                           column named <column>.
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
    flag_exact: bool,
    flag_flag: Option<String>,
    flag_input: Option<String>,
}

impl Args {
    fn get_matcher(&self) -> Result<Matcher, CliError> {
        match self.arg_column.as_ref() {
            None => {
                let pattern = self.arg_pattern.as_ref().unwrap();

                Ok(if self.flag_exact {
                    if self.flag_ignore_case {
                        Matcher::ExactCaseInsensitive(pattern.to_lowercase())
                    } else {
                        Matcher::Exact(pattern.as_bytes().to_vec())
                    }
                } else {
                    Matcher::Regex(
                        RegexBuilder::new(&pattern)
                            .case_insensitive(self.flag_ignore_case)
                            .build()?,
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

                while rdr.read_byte_record(&mut record)? {
                    let pattern = &record[column_index];

                    if self.flag_exact {
                        if self.flag_ignore_case {
                            lower_set.insert(lowercase(pattern));
                        } else {
                            set.insert(pattern.to_vec());
                        }
                    }
                }

                Ok(if self.flag_exact {
                    if self.flag_ignore_case {
                        Matcher::ManyExactCaseInsensitive(lower_set)
                    } else {
                        Matcher::ManyExact(set)
                    }
                } else {
                    unimplemented!()
                })
            }
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
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
