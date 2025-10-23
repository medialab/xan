use bstr::ByteSlice;
use csv::ByteRecord;
use regex::bytes::Regex;

use crate::config::{Config, Delimiter};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Separate columns into multiple columns by splitting cell values on a separator or regex.
By default, all possible splits are made, but you can limit the number of splits
using the --max-splits option.
Note that by default, the original columns are removed from the output. Use the --keep-column
flag to retain them.

This command takes the specified columns and splits each cell in those columns using either
a substring separator or a regex pattern. The resulting parts are output as new columns.
You can choose to split by a simple substring or use a regex for more complex splitting.
Additional options allow you to extract only matching parts, or capture groups from the regex.

Examples:

  Split column named 'fullname' on space:
    $ xan separate fullname ' ' data.csv

  Split column named 'fullname' on whitespaces using a regex:
    $ xan separate -r fullname '\s+' data.csv

  Extract digit sequences from column named 'birthdate' as separate columns using a regex:
    $ xan separate -r -m birthdate '\d+' data.csv

  Extract year, month and day from column named 'date' using capture groups:
    $ xan separate date '(\d{4})-(\d{2})-(\d{2})' data.csv -r -c --into year,month,day

Usage:
    xan separate [options] <columns> <separator> [<input>]
    xan separate --help

separate options:
    -k, --keep               Keep the separated columns after splitting.
    --max-splits <n>         Limit the number of splits per cell to at most <n>.
                             By default, all possible splits are made.
    --into <col1,col2,...>   Specify names for the new columns created by the
                             splits. If not provided, new columns will be named
                             split1, split2, etc. If used with --max-splits,
                             the number of names provided must be equal or lower
                             than <n>.
    --extra <option>         Specify how to handle extra splits when the number
                             of splits exceeds --max-splits, or the number of
                             provided names with --into. By default, it will
                             cause an error. Options are 'drop' to discard extra
                             parts, or 'merge' to combine them into the last column.
    -r, --regex              Split cells using a regex pattern instead of the
                             <separator> substring.
    -m, --match              When using --regex, only output the parts of the
                             cell that match the regex pattern. By default, the
                             parts between matches (i.e. separators) are output.
    -c, --capture-groups     When using --regex, if the regex contains capture
                             groups, output the text matching each capture group
                             as a separate column.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize, Debug)]
struct Args {
    arg_columns: SelectColumns,
    arg_separator: String,
    arg_input: Option<String>,
    flag_regex: bool,
    flag_match: bool,
    flag_capture_groups: bool,
    flag_keep: bool,
    flag_max_splits: Option<usize>,
    flag_into: Option<String>,
    flag_extra: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

enum RegexMode {
    Split,
    Match,
    CaptureGroups,
}

enum Splitter {
    Substring(Vec<u8>),
    Regex(Regex, RegexMode),
}

impl Splitter {
    fn count_splits<'a>(&self, cells: impl Iterator<Item = &'a [u8]>) -> usize {
        let mut count = 0;

        for cell in cells {
            count += match self {
                Self::Substring(sep) => cell.find_iter(sep).count() + 1,
                Self::Regex(pattern, mode) => match mode {
                    RegexMode::Split => pattern.find_iter(cell).count() + 1,
                    RegexMode::CaptureGroups => {
                        pattern.captures_iter(cell).map(|m| m.len() - 1).sum()
                    }
                    RegexMode::Match => pattern.find_iter(cell).count(),
                },
            };
        }

        count
    }

    fn split<'s, 'c>(&'s self, cell: &'c [u8]) -> Box<dyn Iterator<Item = &'c [u8]> + 's>
    where
        'c: 's,
    {
        match self {
            Self::Substring(sep) => Box::new(cell.split_str(sep)),
            Self::Regex(pattern, mode) => match mode {
                RegexMode::Split => Box::new(pattern.split(cell)),
                RegexMode::CaptureGroups => {
                    Box::new(pattern.captures_iter(cell).flat_map(|caps| {
                        caps.iter()
                            .skip(1)
                            .map(|m| m.map(|b| b.as_bytes()).unwrap_or(b""))
                            .collect::<Vec<_>>()
                    }))
                }
                RegexMode::Match => Box::new(pattern.find_iter(cell).map(|m| m.as_bytes())),
            },
        }
    }
}

fn output_splits(
    record: &ByteRecord,
    mut max_splits: usize,
    splitter: &Splitter,
    sel: &Selection,
    extra: &Option<String>,
) -> CliResult<ByteRecord> {
    let mut output_record = ByteRecord::new();
    let expected_num_fields = max_splits + output_record.len();
    let mut matches_to_add: Box<dyn Iterator<Item = &[u8]>>;

    for cell in sel.select(record) {
        matches_to_add = splitter.split(cell);

        if extra.is_some() {
            // Collect all parts into a Vec so we can take some and optionally merge the rest.
            let parts: Vec<Vec<u8>> = matches_to_add.map(|s| s.to_vec()).collect();
            let take_n = std::cmp::min(parts.len(), max_splits);
            output_record.extend(parts.iter().take(take_n - 1).inspect(|_| {
                max_splits = max_splits.saturating_sub(1);
            }));

            let mut last_cell: Vec<u8> = Vec::new();

            match extra.as_ref().unwrap().as_str() {
                "merge" => {
                    // If we've exhausted the slots (max_splits == 1, one last
                    // cell to add) AND there are remaining parts, merge them
                    // into a single field separated by a pipe.
                    if max_splits == 1 && parts.len() > take_n {
                        for (idx, part) in parts.iter().enumerate().skip(take_n - 1) {
                            if idx > take_n - 1 {
                                last_cell.push(b'|');
                            }
                            last_cell.extend_from_slice(part);
                        }
                    } else {
                        last_cell = parts.iter().take(take_n).last().unwrap().clone();
                    }
                }
                _ => {
                    last_cell = parts.iter().take(take_n).last().unwrap().clone();
                }
            }
            output_record.push_field(&last_cell);
            max_splits = max_splits.saturating_sub(1);
        } else {
            output_record.extend(matches_to_add);
        }
    }

    if output_record.len() > expected_num_fields {
        Err("Number of splits exceeded the given maximum. Consider using the --extra flag to handle extra splits.")?;
    }

    while max_splits > 0 && output_record.len() < expected_num_fields {
        output_record.push_field(b"");
        max_splits -= 1;
    }

    Ok(output_record)
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_capture_groups || args.flag_match {
        if !args.flag_regex {
            Err("--capture-groups and --match can only be used with --regex")?;
        }
        if args.flag_capture_groups && args.flag_match {
            Err("--capture-groups and --match cannot be used together")?;
        }
    }

    if args.flag_extra.is_some() {
        if args.flag_max_splits.is_none() && args.flag_into.is_none() {
            Err("--extra can only be used with --max-splits or --into")?;
        }
    } else if args.flag_into.is_some()
        && args.flag_max_splits.is_some()
        && util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len()
            > args.flag_max_splits.unwrap()
    {
        Err("--into cannot specify more column names than --max-splits")?;
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let sel = rconf.selection(&headers)?;

    let mut records: Vec<ByteRecord> = Vec::new();
    let mut max_splits: usize = 0;

    let splitter = if args.flag_regex {
        let regex_mode = if args.flag_match {
            RegexMode::Match
        } else if args.flag_capture_groups {
            RegexMode::CaptureGroups
        } else {
            RegexMode::Split
        };

        Splitter::Regex(Regex::new(&args.arg_separator)?, regex_mode)
    } else {
        Splitter::Substring(args.arg_separator.as_bytes().to_vec())
    };

    // When we need to determine the maximum number of splits across all rows
    // (to know how many new columns to create), we have to first read all records
    // and store them in memory.
    if let Some(n) = args.flag_max_splits {
        max_splits = n;
    } else if args.flag_into.is_some() {
        max_splits = util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len();
    } else {
        for result in rdr.byte_records() {
            let record = result?;

            let numsplits = splitter.count_splits(sel.select(&record));
            max_splits = max_splits.max(numsplits);

            records.push(record);
        }
    }

    let mut sel_to_keep: Selection = rconf.selection(&headers)?;

    if !args.flag_keep {
        sel_to_keep = sel_to_keep.inverse(headers.len());
    }

    let mut new_headers: ByteRecord;
    if args.flag_keep {
        new_headers = headers.clone();
    } else {
        new_headers = sel_to_keep.select(&headers).collect::<ByteRecord>()
    }
    let mut number_of_new_columns = max_splits;
    if let Some(into) = &args.flag_into {
        let headers_to_add = util::str_to_csv_byte_record(into);
        new_headers.extend(&headers_to_add);
        number_of_new_columns -= headers_to_add.len();
    }

    for i in 1..=number_of_new_columns {
        let header_name = format!("split{}", i);
        new_headers.push_field(header_name.as_bytes());
    }
    wtr.write_byte_record(&new_headers)?;

    if args.flag_max_splits.is_some() || args.flag_into.is_some() {
        let mut record = ByteRecord::new();
        while rdr.read_byte_record(&mut record)? {
            let mut output_record = sel_to_keep.select(&record).collect::<ByteRecord>();
            output_record.extend(&output_splits(
                &record,
                max_splits,
                &splitter,
                &sel,
                &args.flag_extra,
            )?);
            wtr.write_byte_record(&output_record)?;
        }
    } else {
        for record in records {
            let mut output_record = sel_to_keep.select(&record).collect::<ByteRecord>();
            output_record.extend(&output_splits(
                &record,
                max_splits,
                &splitter,
                &sel,
                &args.flag_extra,
            )?);
            wtr.write_byte_record(&output_record)?;
        }
    }
    Ok(wtr.flush()?)
}
