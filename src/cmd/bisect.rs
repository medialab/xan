use simd_csv::ByteRecord;

use crate::cmd::sort::{compare_num, parse_num, Number};
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

Usage:
    xan bisect [options] [--] <column> <value> [<input>]
    xan bisect --help

complete options:
    -N, --numeric            Compare according to the numerical value of cells
                             instead of the default lexicographic order.

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
    let mut record_pos: u64;
    let mut median_byte_for_first_occurrence: Option<u64> = None;

    let first_record = seek_rdr.first_byte_record()?.unwrap();
    let last_record = seek_rdr.last_byte_record()?.unwrap();
    let first_value = if args.flag_numeric {
        ValuesType::new_number(&first_record[column_index])
    } else {
        ValuesType::new_string(&first_record[column_index])
    };
    if target_value == first_value {
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
    } else {
        let last_value = if args.flag_numeric {
            ValuesType::new_number(&last_record[column_index])
        } else {
            ValuesType::new_string(&last_record[column_index])
        };
        if target_value >= first_value || target_value <= last_value {
            while start_byte <= end_byte {
                let sought = seek_rdr.find_record_after(median_byte)?;

                // Meaning we reached the last record
                if sought.is_none() {
                    break;
                }

                (record_pos, record) = sought.unwrap();

                value = if args.flag_numeric {
                    ValuesType::new_number(&record[column_index])
                } else {
                    ValuesType::new_string(&record[column_index])
                };

                match value.cmp(&target_value) {
                    std::cmp::Ordering::Equal => {
                        // We need to find the first occurrence of the target value
                        end_byte = record_pos;
                        median_byte_for_first_occurrence =
                            if let Some(pos) = median_byte_for_first_occurrence {
                                Some(std::cmp::min(pos, median_byte))
                            } else {
                                Some(median_byte)
                            };
                    }
                    std::cmp::Ordering::Less => {
                        // move start byte up
                        start_byte = median_byte;
                    }
                    std::cmp::Ordering::Greater => {
                        // move end byte down
                        end_byte = median_byte;
                    }
                }

                if let Some(prev) = previous_median {
                    // We are not making any more progress
                    if prev == median_byte {
                        // Checking the second record as it is skipped when records are too small
                        let mut rdr = rconf.simd_reader()?;

                        let second_post = rdr.byte_records().nth(1);
                        if let Some(second_post) = second_post {
                            let second_record = second_post?;
                            let second_value = if args.flag_numeric {
                                ValuesType::new_number(&second_record[column_index])
                            } else {
                                ValuesType::new_string(&second_record[column_index])
                            };
                            if second_value == target_value {
                                wtr.write_byte_record(&second_record)?;
                                median_byte_for_first_occurrence = Some(
                                    seek_rdr.first_record_position()
                                        + first_record.as_slice().len() as u64
                                        + 1,
                                );
                            }
                        }

                        if let Some(pos) = median_byte_for_first_occurrence {
                            // We found at least one occurrence of the target value
                            // Now we need to write all occurrences
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
                                gap = if record_pos == old_record_pos {
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

                        break;
                    }
                }
                previous_median = Some(median_byte);
                median_byte = (start_byte + end_byte) / 2;
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
