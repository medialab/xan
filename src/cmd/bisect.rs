use std::cmp::Ordering;
use std::io::SeekFrom;

use simd_csv::ByteRecord;

use crate::cmd::sort::{compare_num, parse_num, Number};
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

#[derive(Clone, PartialEq, Debug)]
enum Value {
    Number(Number),
    String(Vec<u8>),
}

impl Eq for Value {}

impl PartialOrd for Value {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Value {
    #[inline]
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
            None => Err(format!(
                "Failed to parse {} as a number!",
                std::str::from_utf8(s).unwrap()
            )),
        }
    }
}

static USAGE: &str = r#"
Perform binary search on sorted CSV data.

This command is one order of magnitude faster than relying on `xan filter` or
`xan search` but only works if target file is sorted on searched column, exists
on disk and is not compressed (unless the compressed file remains seekable,
typically if some `.gzi` index can be found beside it).

If CSV data is not properly sorted, result will be incorrect!

By default this command executes the so-called "lower bound" operation: it
positions itself in the file where one would insert the searched value and then
proceeds to flush the file from this point. This can be useful when piping
into other commands to perform range queries, for instance, or enumerate values
starting with some prefix.

Use the -S/--search flag if you only want to return rows matching your query
exactly.

Finally, use the -R/--reverse flag if data is sorted in descending order and
the -N/--numeric flag if data is sorted numerically rather than lexicographically.

Examples:

Searching for rows matching exactly "Anna" in a "name" column:

    $ xan bisect -S name Anna people.csv

Finding all names starting with letter A:

    $ xan bisect name A people.csv | xan slice -E '!name.startswith("A")'

Usage:
    xan bisect [options] [--] <column> <value> <input>
    xan bisect --help

bisect options:
    -S, --search   Perform an exact search and only emit rows matching the
                   query, instead of flushing all rows from found position.
    -R, --reverse  Indicate that the file is sorted on <column> in descending
                   order, instead of the default ascending order.
    -N, --numeric  Indicate that searched values are numbers and that the order
                   of the file is numerical instead of default lexicographic
                   order.
    -E, --exclude  When set, rows matching query exactly will be filtered out.
                   It is equivalent to performing the "upper bound" operation
                   but it does not come with the same performance guarantees
                   in case there are many rows containing the searched values.
                   Does not work with -S/--search.
    -v, --verbose  Print some log detailing the search process in stderr, mostly
                   for debugging purposes.

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
    flag_exclude: bool,
    flag_numeric: bool,
    flag_reverse: bool,
    flag_search: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_verbose: bool,
}

impl Args {
    #[inline]
    fn get_value_from_bytes(&self, bytes: &[u8]) -> Result<Value, String> {
        if self.flag_numeric {
            Value::new_number(bytes)
        } else {
            Ok(Value::new_string(bytes))
        }
    }

    #[inline]
    fn cmp(&self, v1: &Value, v2: &Value) -> Ordering {
        let ordering = v1.cmp(v2);

        if self.flag_reverse {
            ordering.reverse()
        } else {
            ordering
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_exclude && args.flag_search {
        Err("The -E/--exclude and -S/--search flags cannot be used together")?;
    }

    macro_rules! log {
        ($($arg:tt)*) => {
            if args.flag_verbose {
                eprintln!($($arg)*);
            }
        };
    }

    let searched_value = args.get_value_from_bytes(args.arg_value.as_bytes())?;

    let rconf = Config::new(&Some(args.arg_input.clone()))
        .no_headers(args.flag_no_headers)
        .select(args.arg_column.clone())
        .delimiter(args.flag_delimiter);

    let mut seeker = rconf.simd_seeker()?.ok_or("File cannot be seeked!")?;
    let column_index = rconf.single_selection(seeker.byte_headers())?;

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconf.no_headers {
        wtr.write_byte_record(seeker.byte_headers())?;
    }

    let first_record = match seeker.first_byte_record()? {
        Some(r) => r,
        None => {
            // NOTE: file is empty!
            return Ok(());
        }
    };

    let last_record = seeker.last_byte_record()?.unwrap();

    let first_value = args.get_value_from_bytes(&first_record[column_index])?;
    let last_value = args.get_value_from_bytes(&last_record[column_index])?;

    let mut lo = seeker.first_record_position();
    let mut hi = seeker.stream_len();

    log!("lo byte: {}", lo);
    log!("hi byte: {}", hi);

    // File does not seem to be correctly sorted
    if args.cmp(&first_value, &last_value).is_gt() {
        Err(format!(
            "input is not sorted in specified order!\nSee first and last values: {} and {}",
            first_value, last_value
        ))?;
    }

    // Searched value is more than last value: we can stop right now
    if args.cmp(&searched_value, &last_value).is_gt() {
        log!("early exit: search value is after last value!");
        return Ok(());
    }

    // Searched value is less than first value or equal
    let mut skip_search = false;

    if args.cmp(&searched_value, &first_value).is_le() {
        log!("skipping search: search value is before first value!");
        skip_search = true;
    }

    // `bisect_left`
    // while lo < hi:
    //     mid = (lo+hi)//2
    //     if a[mid] < x: lo = mid+1
    //     else: hi = mid
    // return lo

    let mut jumps: usize = 0;

    if !skip_search {
        while lo < hi {
            let mid = (lo + hi) / 2;
            log!("\nmid byte: {}", mid);

            jumps += 1;

            match seeker.find_record_after(mid)? {
                Some((pos, record)) => {
                    log!("successful jump n°{} to: {} (+{})", jumps, pos, pos - mid);

                    let value = args.get_value_from_bytes(&record[column_index])?;

                    log!("found value: {}", value);

                    match args.cmp(&value, &searched_value) {
                        Ordering::Less => {
                            lo = mid + 1;
                            log!("new lo (going right): {}", lo);
                        }
                        _ => {
                            hi = mid;
                            log!("new hi (going left): {}", hi);
                        }
                    }

                    // Is there enough space for next jump to make sense?
                    let next_mid = (lo + hi) / 2;

                    // NOTE: should we multiply lookahead len by 2?
                    // The question is to balance the number of jumps vs.
                    // the number of subsequent skips.
                    if next_mid.abs_diff(mid) <= seeker.lookahead_len() {
                        break;
                    }
                }
                None => {
                    Err(format!(
                        "Seeker's lookahead failed (len: {}, pos: {})!",
                        seeker.lookahead_len(),
                        mid
                    ))?;
                }
            }
        }
    }

    log!("\nfinal lo: {}", lo);
    log!(
        "made {} jumps vs. expected log(n) {}",
        jumps,
        (seeker.approx_count() as f64).log2().ceil() as usize
    );

    let final_pos = if skip_search {
        seeker.first_record_position()
    } else {
        seeker.find_record_after(lo)?.unwrap().0
    };

    let mut reader = seeker.into_reader_at_position(SeekFrom::Start(final_pos))?;

    let mut record = ByteRecord::new();
    let mut skipped: usize = 0;
    let mut logged_skipped: bool = false;

    while reader.read_byte_record(&mut record)? {
        let value = args.get_value_from_bytes(&record[column_index])?;

        match args.cmp(&value, &searched_value) {
            Ordering::Less => {
                skipped += 1;
            }
            Ordering::Equal => {
                if !args.flag_exclude {
                    if !logged_skipped {
                        log!("skipped records before finding: {}", skipped);
                        logged_skipped = true;
                    }
                    wtr.write_byte_record(&record)?;
                } else {
                    skipped += 1;
                }
            }
            Ordering::Greater => {
                if args.flag_exclude && !logged_skipped {
                    log!("skipped records before finding: {}", skipped);
                    logged_skipped = true;
                }

                if args.flag_search {
                    break;
                } else {
                    wtr.write_byte_record(&record)?;
                }
            }
        }
    }

    Ok(wtr.flush()?)
}
