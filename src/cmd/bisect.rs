use simd_csv::ByteRecord;

use crate::cmd::sort::{compare_num, parse_num, Number};
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Search for rows where the value in <column> matches <value> using binary search.
It is assumed that the INPUT IS SORTED according to the specified column.
The ordering of the rows is assumed to be sorted according ascending lexicographic
order per default, but you can specify numeric ordering using the -N or --numeric
flag. You can also reverse the order using the -R/--reverse flag.

Usage:
    xan bisect [options] [--] <column> <value> [<input>]
    xan bisect --help

complete options:
    -N, --numeric            Compare according to the numerical value of cells
                             instead of the default lexicographic order.
    -R, --reverse            Reverse sort order, i.e. descending order.

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
    arg_column: SelectedColumns,
    arg_value: String,
    arg_input: Option<String>,
    flag_numeric: bool,
    flag_reverse: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

#[derive(Clone, PartialEq, Debug)]
enum ValuesType {
    Number(Number),
    String(String),
}

impl Eq for ValuesType {}

impl PartialOrd for ValuesType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ValuesType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (ValuesType::Number(n1), ValuesType::Number(n2)) => compare_num(*n1, *n2),
            (ValuesType::String(s1), ValuesType::String(s2)) => s1.cmp(s2),
            _ => panic!("Cannot compare different value types"),
        }
    }
}

impl ValuesType {
    fn new_string(s: &[u8]) -> Self {
        ValuesType::String(std::str::from_utf8(s).unwrap().to_string())
    }

    fn new_number(s: &[u8]) -> Self {
        match parse_num(s) {
            Some(n) => ValuesType::Number(n),
            None => panic!("Failed to parse number from bytes"),
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let target_value = if args.flag_numeric {
        ValuesType::new_number(args.arg_value.as_bytes())
    } else {
        ValuesType::new_string(args.arg_value.as_bytes())
    };

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column)
        .delimiter(args.flag_delimiter);

    let mut seek_rdr = rconf.simd_seeker()?.unwrap();

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconf.no_headers {
        wtr.write_byte_record(rconf.simd_reader()?.byte_headers()?)?;
    }

    let column_index = rconf.single_selection(rconf.reader()?.byte_headers()?)?;

    let mut median_byte = seek_rdr.stream_len() / 2;
    let mut start_byte = seek_rdr.first_record_position();
    let mut end_byte = seek_rdr.stream_len();

    let mut previous_median: Option<u64> = None;

    let mut value: ValuesType;

    let mut record: ByteRecord;
    let mut record_pos: u64 = seek_rdr.first_record_position();
    let mut median_byte_for_first_occurrence: Option<u64> = None;

    let first_record = seek_rdr.first_byte_record()?.unwrap();
    let last_record = seek_rdr.last_byte_record()?.unwrap();
    let first_value = if args.flag_numeric {
        ValuesType::new_number(&first_record[column_index])
    } else {
        ValuesType::new_string(&first_record[column_index])
    };

    match (args.flag_reverse, target_value.cmp(&first_value)) {
        (_, std::cmp::Ordering::Equal) => {
            // No search needed, write first record and every next ones with the same value
            // record_pos = seek_rdr.first_record_position();
            let mut rdr = rconf.simd_reader()?;
            record = ByteRecord::new();

            while rdr.read_byte_record(&mut record)? {
                if {
                    if args.flag_numeric {
                        ValuesType::new_number(&record[column_index])
                    } else {
                        ValuesType::new_string(&record[column_index])
                    }
                } == target_value
                {
                    wtr.write_byte_record(&record)?;
                } else {
                    break;
                }
            }

            return Ok(wtr.flush()?);
        }
        (false, std::cmp::Ordering::Less) | (true, std::cmp::Ordering::Greater) => {
            // Target value is out of bounds, so it is not present
        }
        (false, std::cmp::Ordering::Greater) | (true, std::cmp::Ordering::Less) => {
            // checking the second record as it is skipped
            // by simd_csv::Seek::Seeker::find_record_after()
            // when records are too small
            let mut rdr = rconf.simd_reader()?;

            if let Some(second_record) = rdr.byte_records().nth(1) {
                let second_record = second_record?;
                let second_value = if args.flag_numeric {
                    ValuesType::new_number(&second_record[column_index])
                } else {
                    ValuesType::new_string(&second_record[column_index])
                };
                if target_value == second_value {
                    wtr.write_byte_record(&second_record)?;

                    for record in rdr.byte_records() {
                        let record = record?;
                        let value = if args.flag_numeric {
                            ValuesType::new_number(&record[column_index])
                        } else {
                            ValuesType::new_string(&record[column_index])
                        };
                        if value == target_value {
                            wtr.write_byte_record(&record)?;
                        } else {
                            break;
                        }
                    }

                    return Ok(wtr.flush()?);
                }
            }
        }
    }

    let last_value = if args.flag_numeric {
        ValuesType::new_number(&last_record[column_index])
    } else {
        ValuesType::new_string(&last_record[column_index])
    };

    if match args.flag_reverse {
        false => target_value >= first_value && target_value <= last_value,
        true => target_value <= first_value && target_value >= last_value,
    } {
        while start_byte <= end_byte {
            let sought = seek_rdr.find_record_after(median_byte)?;

            if let Some(sought) = sought {
                (record_pos, record) = sought;
                value = if args.flag_numeric {
                    ValuesType::new_number(&record[column_index])
                } else {
                    ValuesType::new_string(&record[column_index])
                };

                match (args.flag_reverse, value.cmp(&target_value)) {
                    (_, std::cmp::Ordering::Equal) => {
                        // We need to find the first occurrence of the target value
                        end_byte = record_pos - 1;
                        median_byte_for_first_occurrence =
                            if let Some(pos) = median_byte_for_first_occurrence {
                                Some(std::cmp::min(pos, median_byte))
                            } else {
                                Some(median_byte)
                            };
                    }
                    (false, std::cmp::Ordering::Less) | (true, std::cmp::Ordering::Greater) => {
                        // move start byte up
                        start_byte = median_byte;
                    }
                    (false, std::cmp::Ordering::Greater) | (true, std::cmp::Ordering::Less) => {
                        // move end byte down
                        end_byte = median_byte;
                    }
                }
            } else {
                // Meaning we reached the last record or there are no more records
                // after median_byte, so we try to find some before it
                end_byte = median_byte;
                median_byte = (start_byte + end_byte) / 2;
                continue;
            }

            if let Some(prev) = previous_median {
                // We are not making any more progress
                // Meaning the target value is either found or not present
                if prev == median_byte {
                    break;
                }
            }
            previous_median = Some(median_byte);
            median_byte = (start_byte + end_byte) / 2;
        }

        // We found at least one (and the first) occurrence of
        // the target value, so now we need to write all occurrences
        if let Some(pos) = median_byte_for_first_occurrence {
            let mut pos = pos;
            let mut gap: u64;
            let mut returned_first_occurrence = false;
            loop {
                let sought = seek_rdr.find_record_after(pos)?;
                if sought.is_none() {
                    break;
                }

                let old_record_pos = record_pos;
                (record_pos, record) = sought.unwrap();
                gap = if record_pos <= old_record_pos {
                    record.as_slice().len() as u64 + 1
                } else {
                    record_pos - old_record_pos
                };

                // Check if we have a new record and if we have not yet returned
                // the first occurrence
                if record_pos != old_record_pos || !returned_first_occurrence {
                    value = if args.flag_numeric {
                        ValuesType::new_number(&record[column_index])
                    } else {
                        ValuesType::new_string(&record[column_index])
                    };

                    if value == target_value {
                        wtr.write_byte_record(&record)?;
                        returned_first_occurrence = true;
                    } else {
                        break;
                    }
                }
                pos += gap;
            }
        }

        if target_value == last_value {
            // Note: the '-1' is to account for the final newline
            // record_pos = end_byte - last_record.as_slice().len() as u64 - 1;
            wtr.write_byte_record(&last_record)?;
        }
    }

    Ok(wtr.flush()?)
}
