use std::cmp::Ordering;
use std::io::SeekFrom;

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
    xan bisect [options] [--] <column> <value> <input>
    xan bisect --help

bisect options:
    -I, --include            When set, the records with the target value will be
                             included in the output. By default, it is not.
                             Cannot be used with -S/--search.
    -N, --numeric            Compare according to the numerical value of cells
                             instead of the default lexicographic order.
    -R, --reverse            Reverse sort order, i.e. descending order.
    -S, --search             Perform a search on the target value instead of
                             flushing all records after the value (included).
                             Cannot be used with -I/--include.

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
    arg_input: String,
    flag_include: bool,
    flag_numeric: bool,
    flag_reverse: bool,
    flag_search: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn get_value_from_bytes(&self, bytes: &[u8]) -> Result<Value, String> {
        if self.flag_numeric {
            Value::new_number(bytes)
        } else {
            Ok(Value::new_string(bytes))
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
enum Value {
    Number(Number),
    String(Vec<u8>),
}

impl Eq for Value {}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(n1), Self::Number(n2)) => compare_num(*n1, *n2),
            (Self::String(s1), Self::String(s2)) => s1.cmp(s2),
            _ => panic!("Cannot compare different value types"),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Number(n) => match n {
                Number::Int(i) => write!(f, "{}", i),
                Number::Float(fl) => write!(f, "{}", fl),
            },
            Self::String(s) => write!(f, "{}", std::str::from_utf8(s).unwrap()),
        }
    }
}

impl Value {
    fn new_string(s: &[u8]) -> Self {
        Self::String(s.to_vec())
    }

    fn new_number(s: &[u8]) -> Result<Self, String> {
        match parse_num(s) {
            Some(n) => Ok(Self::Number(n)),
            None => Err("Failed to parse number from bytes".to_string()),
        }
    }
}

fn reversing_order_if_necessary(ord: Ordering, reverse: bool) -> Ordering {
    if reverse {
        ord.reverse()
    } else {
        ord
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_include && args.flag_search {
        Err("The -I/--include and -S/--search flags cannot be used together")?;
    }

    let target_value = args.get_value_from_bytes(args.arg_value.as_bytes())?;

    let rconf = Config::new(&Some(args.arg_input.clone()))
        .no_headers(args.flag_no_headers)
        .select(args.arg_column.clone())
        .delimiter(args.flag_delimiter);

    let mut seek_rdr = rconf.simd_seeker()?.unwrap();

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconf.no_headers {
        wtr.write_byte_record(seek_rdr.byte_headers())?;
    }

    let column_index = rconf.single_selection(seek_rdr.byte_headers())?;

    let mut start_byte = seek_rdr.first_record_position();
    let mut end_byte = seek_rdr.stream_len();
    let mut median_byte = (start_byte + end_byte) / 2;

    let mut previous_median: Option<u64> = None;

    let mut previous_median_value: Option<Value> = None;
    let mut being_in_first_half: bool = false;

    let mut record = ByteRecord::new();
    let mut record_pos: u64 = seek_rdr.first_record_position();
    let mut previously_found_record_pos: Option<u64> = None;
    let mut median_byte_for_first_occurrence: Option<u64> = None;

    let first_record = seek_rdr.first_byte_record()?.unwrap();
    let last_record = seek_rdr.last_byte_record()?.unwrap();

    let first_value = args.get_value_from_bytes(&first_record[column_index])?;
    let last_value = args.get_value_from_bytes(&last_record[column_index])?;

    if reversing_order_if_necessary(first_value.cmp(&last_value), args.flag_reverse)
        == Ordering::Greater
    {
        Err(
            format!("Input is not sorted in the specified order, first and last values are inconsistent: {} and {}", first_value, last_value)
        )?;
    }

    // Testing the first and second records before starting
    match reversing_order_if_necessary(first_value.cmp(&target_value), args.flag_reverse) {
        Ordering::Greater => {
            // Target value is out of bounds, so it is not present
        }
        ord => {
            // Will check first and second records (second too as it is skipped
            // by simd_csv::Seek::Seeker::find_record_after()
            // when records are too small)
            let mut rdr = rconf.simd_reader()?;
            let mut value_found = false;

            if ord == Ordering::Less {
                // Skipping the first record as it is smaller than the target value
                // will still check the second record afterwards in the loop
                rdr.read_byte_record(&mut record)?;
            }

            while rdr.read_byte_record(&mut record)? {
                match reversing_order_if_necessary(
                    args.get_value_from_bytes(&record[column_index])?
                        .cmp(&target_value),
                    args.flag_reverse,
                ) {
                    Ordering::Equal => {
                        value_found = true;
                        if !args.flag_include && !args.flag_search {
                            continue;
                        }
                    }
                    ord => {
                        if args.flag_search || ord == Ordering::Less {
                            break;
                        }
                    }
                }
                wtr.write_byte_record(&record)?;
            }

            if value_found {
                return Ok(wtr.flush()?);
            }
        }
    }

    if match args.flag_reverse {
        false => target_value >= first_value && target_value <= last_value,
        true => target_value <= first_value && target_value >= last_value,
    } {
        while start_byte <= end_byte {
            let sought = seek_rdr.find_record_after(median_byte)?;

            if let Some(sought) = sought {
                (record_pos, record) = sought;
                let value = args.get_value_from_bytes(&record[column_index])?;
                match reversing_order_if_necessary(value.cmp(&target_value), args.flag_reverse) {
                    Ordering::Equal => {
                        // We need to find the first occurrence of the target value
                        end_byte = record_pos - 1;
                        being_in_first_half = true;
                        median_byte_for_first_occurrence =
                            if let Some(pos) = median_byte_for_first_occurrence {
                                Some(std::cmp::min(pos, median_byte))
                            } else {
                                Some(median_byte)
                            };
                    }
                    ord => {
                        if let Some(prev_value) = previous_median_value {
                            match (
                                being_in_first_half,
                                reversing_order_if_necessary(
                                    prev_value.cmp(&value),
                                    args.flag_reverse,
                                ),
                            ) {
                                (true, Ordering::Less) => {
                                    Err(format!("Input is not sorted in the specified order, inconsistent values found during search: value {} in record on byte {} comes after {} in record on byte {}", prev_value, previously_found_record_pos.unwrap_or(0), value, record_pos))?;
                                }
                                (false, Ordering::Greater) => {
                                    Err(format!("Input is not sorted in the specified order, inconsistent values found during search: value {} in record on byte {} comes before {} in record on byte {}", prev_value, previously_found_record_pos.unwrap_or(0), value, record_pos))?;
                                }
                                _ => {}
                            }
                        }

                        match ord {
                            Ordering::Less => {
                                // meaning we're looking for a bigger value but the median value is actually the last one of the subset studied,
                                // so the value isn't found
                                if record_pos >= end_byte {
                                    break;
                                }
                                // move start byte up
                                being_in_first_half = false;
                                start_byte = median_byte;
                            }
                            Ordering::Greater => {
                                // move end byte down
                                being_in_first_half = true;
                                end_byte = median_byte;
                            }
                            _ => {}
                        }
                    }
                }
                previous_median_value = Some(value);
                previously_found_record_pos = Some(record_pos);

                if let Some(prev) = previous_median {
                    // We are not making any more progress
                    // Meaning the target value is either found or not present
                    if prev == median_byte {
                        break;
                    }
                }
            } else {
                // Meaning we reached the last record or there are no more records
                // after median_byte, so we try to find some before it
                end_byte = median_byte;
                being_in_first_half = true;
                previous_median_value = None;
                previously_found_record_pos = None;
            }
            previous_median = Some(median_byte);
            median_byte = (start_byte + end_byte) / 2;
        }

        if args.flag_search {
            // We found at least one (and the first) occurrence of
            // the target value, so now we need to write only occurrences of the target value if search flag is set
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
                        let value = args.get_value_from_bytes(&record[column_index])?;

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
        } else {
            // No record found but this is becose only the last record has the target value, so we write it and return
            if median_byte_for_first_occurrence.is_none()
                && target_value == last_value
                && args.flag_include
            {
                // Note: the '-1' is to account for the final newline
                // record_pos = end_byte - last_record.as_slice().len() as u64 - 1;
                wtr.write_byte_record(&last_record)?;
            } else {
                let pos = if let Some(pos) = median_byte_for_first_occurrence {
                    pos
                } else {
                    median_byte
                };
                // Writing every record after the first occurence
                let record_pos = SeekFrom::Start(seek_rdr.find_record_after(pos)?.unwrap().0);
                let mut reader = seek_rdr.into_reader_at_position(record_pos)?;
                let mut record = ByteRecord::new();
                while reader.read_byte_record(&mut record)? {
                    dbg!(&record);
                    let value = args.get_value_from_bytes(&record[column_index])?;
                    match reversing_order_if_necessary(value.cmp(&target_value), args.flag_reverse)
                    {
                        Ordering::Equal => {
                            if !args.flag_include {
                                continue;
                            }
                        }
                        Ordering::Greater => {}
                        // meaning the first records aren't the target value,
                        // happens sometimes
                        Ordering::Less => continue,
                    };
                    wtr.write_byte_record(&record)?;
                }
            }
        }
    }

    Ok(wtr.flush()?)
}
