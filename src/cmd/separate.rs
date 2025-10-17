use bstr::ByteSlice;
use csv::ByteRecord;
use regex::bytes;
use std::iter::Iterator;

use crate::config::{Config, Delimiter};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Separate columns into multiple columns by splitting cell values on a separator or regex.
By default, all possible splits are made, but you can limit the number of splits
using the --max-splits option.
Note that by default, the original columns are removed from the output.

This command takes the specified columns and splits each cell in those columns using either
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
    $ xan separate date '(\d{4})-(\d{2})-(\d{2})' data.csv -r -c --into year,month,day

Usage:
    xan separate [options] <columns> <separator> [<input>]
    xan separate --help

separate options:
    --keep-column            Keep the original column after splitting.
    --max-splits <n>         Limit the number of splits per cell to at most <n>.
                             By default, all possible splits are made.
    --into <col1,col2,...>   Specify names for the new columns created by the splits.
                             If not provided, new columns will be named untitled1, 
                             untitled2, etc. If used with --max-splits, the number
                             of names provided must be equal or lower than <n>.
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
    flag_keep_column: bool,
    flag_max_splits: Option<usize>,
    flag_into: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

enum Splitter {
    String(Vec<u8>),
    Regex(bytes::Regex),
}

fn output_splits(
    record: &ByteRecord,
    max_splits: usize,
    keep_column: bool,
    splitter: &Splitter,
    sel: Selection,
    match_only: bool,
    capture_groups: bool,
) -> ByteRecord {
    let mut output_record: ByteRecord = ByteRecord::new();
    if keep_column {
        output_record = record.clone();
    } else {
        for i in 0..record.len() {
            if !sel.contains(i) {
                output_record.push_field(record.get(i).unwrap());
            }
        }
    }
    let expected_num_fields = max_splits + output_record.len();

    for cell in sel.select(record) {
        match &splitter {
            Splitter::Regex(re) => {
                if match_only {
                    output_record.extend(re.find_iter(cell).map(|mat| mat.as_bytes().to_vec()));
                } else if capture_groups {
                    for mat in re.captures_iter(cell) {
                        output_record.extend(
                            &mut mat.iter().skip(1).map(|m| m.unwrap().as_bytes().to_vec()),
                        );
                    }
                } else {
                    output_record.extend(re.split(cell).map(|s| s.to_vec()));
                }
            }
            Splitter::String(separator) => {
                output_record.extend(cell.split_str(separator).map(|s| s.to_vec()));
            }
        }
    }
    while output_record.len() < expected_num_fields {
        output_record.push_field(b"");
    }
    output_record
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

    if args.flag_into.is_some() && args.flag_max_splits.is_some() {
        if args.flag_into.as_ref().unwrap().split(',').count() > args.flag_max_splits.unwrap() {
            Err("--into cannot specify more column names than --max-splits")?;
        }
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

    let splitter: Splitter = match args.flag_regex {
        true => Splitter::Regex(bytes::Regex::new(&args.arg_separator)?),
        false => Splitter::String(args.arg_separator.as_bytes().to_vec()),
    };

    // When we need to determine the maximum number of splits across all rows
    // (to know how many new columns to create), we have to first read all records
    // and store them in memory.
    if let Some(_max_splits) = args.flag_max_splits {
        max_splits = _max_splits;
    } else {
        for result in rdr.byte_records() {
            let record = result?;

            let mut numsplits = 0;
            for cell in sel.select(&record) {
                // dbg!(std::str::from_utf8(cell).ok());
                match &splitter {
                    Splitter::Regex(re) => {
                        if args.flag_match {
                            numsplits += re.find_iter(cell).count();
                        } else if args.flag_capture_groups {
                            for mat in re.captures_iter(cell) {
                                numsplits += mat.len() - 1; // mat[0] is the full match
                            }
                        } else {
                            // Default behavior: split on the regex matches, acting as a separator
                            numsplits += re.find_iter(cell).count() + 1;
                        }
                        if numsplits > max_splits {
                            max_splits = numsplits;
                        }
                        // dbg!(numsplits);
                    }
                    Splitter::String(sep) => {
                        numsplits += cell.find_iter(sep).count() + 1;
                        if numsplits > max_splits {
                            max_splits = numsplits;
                        }
                    }
                };
            }
            records.push(record);
        }
    }

    let mut new_headers: ByteRecord = ByteRecord::new();
    if args.flag_keep_column {
        new_headers = headers.clone();
    } else {
        for i in 0..headers.len() {
            if !sel.contains(i) {
                new_headers.push_field(headers.get(i).unwrap());
            }
        }
    }
    let mut number_of_new_columns = max_splits;
    if let Some(into) = &args.flag_into {
        let new_headers_names: Vec<&str> = into.split(',').collect();
        for name in new_headers_names {
            new_headers.push_field(name.as_bytes());
            number_of_new_columns -= 1;
        }
    }

    for i in 1..=number_of_new_columns {
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
                args.flag_keep_column,
                &splitter,
                sel.clone(),
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
                args.flag_keep_column,
                &splitter,
                sel.clone(),
                args.flag_match,
                args.flag_capture_groups,
            );
            wtr.write_byte_record(&output_record)?;
        }
    }
    Ok(wtr.flush()?)
}
