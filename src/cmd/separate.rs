use bstr::ByteSlice;
use csv::ByteRecord;
use regex::bytes;
use std::iter::Iterator;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Separate one column into multiple columns by splitting cell values on a separator or regex.

This command takes the specified column and splits each cell in those columns using either
a substring separator or a regex pattern. The resulting parts are output as new columns. 
You can choose to split by a simple substring or use a regex for more complex splitting. 
Additional options allow you to extract only matching parts, or capture groups from the regex.

Examples:

  Split column named 2 on commas:
    $ xan separate 2 , data.csv

  Split column named 1 on whitespaces using a regex:
    $ xan separate 1 '\s+' data.csv -r

  Extract digit sequences from column named 1 as separate columns using a regex:
    $ xan separate 1 '\d+' data.csv -r -m

  Extract year, month and day from column named 'date' using capture groups:
    $ xan separate date '(\d{4})-(\d{2})-(\d{2})' data.csv -r -c

Usage:
    xan separate [options] <columns> <separator> [<input>]
    xan separate --help

separate options:
    --max-splits <n>         Limit the number of splits per cell to at most <n>.
                             By default, all possible splits are made.
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
    flag_max_splits: Option<usize>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

fn output_splits(
    record: &ByteRecord,
    max_splits: usize,
    separator: &str,
    regex: bool,
    match_only: bool,
    capture_groups: bool,
) -> ByteRecord {
    let mut output_record: ByteRecord = record.clone();
    let mut split_record: Vec<Vec<u8>> = vec![];
    if regex {
        if match_only {
            split_record = bytes::Regex::new(separator)
                .unwrap()
                .find_iter(record.as_slice())
                .map(|mat| mat.as_bytes().to_vec())
                .collect();
        } else if capture_groups {
            for mat in bytes::Regex::new(separator)
                .unwrap()
                .captures_iter(record.as_slice())
            {
                split_record.append(
                    &mut mat
                        .iter()
                        .skip(1)
                        .map(|m| m.unwrap().as_bytes().to_vec())
                        .collect(),
                );
            }
        } else {
            split_record = bytes::Regex::new(separator)
                .unwrap()
                .split(record.as_slice())
                .map(|s| s.to_vec())
                .collect();
        }
    } else {
        split_record = record
            .as_slice()
            .split_str(separator)
            .map(|s| s.to_vec())
            .collect();
    }
    for cell in split_record.iter() {
        output_record.push_field(&cell);
    }
    while output_record.len() <= max_splits {
        output_record.push_field(b"");
    }
    output_record
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_capture_groups || args.flag_match {
        if !args.flag_regex {
            return Err("--capture-groups and --match can only be used with --regex")?;
        }
        if args.flag_capture_groups && args.flag_match {
            return Err("--capture-groups and --match cannot be used together")?;
        }
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let mut records: Vec<ByteRecord> = Vec::new();
    let mut max_splits: usize = 0;

    // When we need to determine the maximum number of splits across all rows
    // (to know how many new columns to create), we have to first read all records
    // and store them in memory.
    if let Some(_max_splits) = args.flag_max_splits {
        max_splits = _max_splits;
    } else {
        if args.flag_regex {
            let re = bytes::Regex::new(&args.arg_separator)?;
            for result in rdr.byte_records() {
                let record = result?;
                let mut numsplits = 0;
                if args.flag_match {
                    numsplits = re.find_iter(record.as_slice()).count();
                } else if args.flag_capture_groups {
                    for mat in re.captures_iter(record.as_slice()) {
                        numsplits += mat.len() - 1; // mat[0] is the full match
                    }
                } else {
                    // Default behavior: split on the regex matches, acting as a separator
                    numsplits = re.find_iter(record.as_slice()).count() + 1;
                }
                if numsplits > max_splits {
                    max_splits = numsplits;
                }
                records.push(record);
            }
        } else {
            for result in rdr.byte_records() {
                let record = result?;
                let numsplits = record.as_slice().find_iter(b" ").count() + 1;
                if numsplits > max_splits {
                    max_splits = numsplits;
                }

                records.push(record);
            }
        }
    }

    let mut new_headers = ByteRecord::from(headers.clone());
    for i in 1..=max_splits {
        let header_name = format!("untitled{}", i);
        new_headers.push_field(header_name.as_bytes());
    }
    wtr.write_byte_record(&new_headers)?;

    if let Some(_) = args.flag_max_splits {
        for result in rdr.byte_records() {
            let record = result?;
            let output_record = output_splits(
                &record,
                max_splits,
                &args.arg_separator,
                args.flag_regex,
                args.flag_match,
                args.flag_capture_groups,
            );
            wtr.write_byte_record(&output_record)?;
        }
    } else {
        for record in records {
            let output_record = output_splits(
                &record,
                max_splits,
                &args.arg_separator,
                args.flag_regex,
                args.flag_match,
                args.flag_capture_groups,
            );
            wtr.write_byte_record(&output_record)?;
        }
    }
    Ok(wtr.flush()?)
}
