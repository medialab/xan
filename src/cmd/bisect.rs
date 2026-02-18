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
Search for rows where the value in <column> matches <value> using binary search,
and flush all records after the target value.
The default behavior is similar to a lower_bound bisection, but you can exclude
records (equivalent to upper_bound) with the target value using the -E/--exclude
flag. It is assumed that the INPUT IS SORTED according to the specified column.
The ordering of the rows is assumed to be sorted according ascending lexicographic
order per default, but you can specify numeric ordering using the -N or --numeric
flag. You can also reverse the order using the -R/--reverse flag.
Use the -S/--search flag to only flush records matching the target value instead
of all records after it.

Usage:
    xan bisect [options] [--] <column> <value> <input>
    xan bisect --help

bisect options:
    -E, --exclude            When set, the records with the target value will be
                             excluded from the output. By default, they are
                             included. Cannot be used with -S/--search.
                             TODO: not equivalent to upper_bound
    -N, --numeric            Compare according to the numerical value of cells
                             instead of the default lexicographic order.
    -R, --reverse            Reverse sort order, i.e. descending order.
    -S, --search             Perform a search on the target value instead of
                             flushing all records after the value (included).
                             Cannot be used with -E/--exclude nor -e/--end.
    -e, --end <end-value>    When set, the records after the target value will be
                             flushed until <end-value> is reached (included).
                             By default, all records after the target value are
                             flushed. Cannot be used with -S/--search.
    -v, --verbose

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
    flag_end_value: Option<String>,
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

    if args.flag_search && args.flag_end_value.is_some() {
        Err("The -S/--search and -e/--end flags cannot be used together")?;
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

    // Early terminations

    // File does not seem to be correctly sorted
    if args.cmp(&first_value, &last_value) == Ordering::Greater {
        Err(format!(
            "input is not sorted in specified order!\nSee first and last values: {} and {}",
            first_value, last_value
        ))?;
    }

    // Searched value is more than last value
    // TODO...

    // Searched value is less than first value
    // TODO...

    // Searched value is one of first values
    // TODO...

    // Searched value is one of last values
    // TODO...

    // `bisect_left`
    // while lo < hi:
    //     mid = (lo+hi)//2
    //     if a[mid] < x: lo = mid+1
    //     else: hi = mid
    // return lo

    let mut jumps: usize = 0;

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

                if next_mid.abs_diff(mid) <= seeker.lookahead_len() * 2 {
                    break;
                }
            }
            None => {
                // TODO: deal with end of file or declare search a failure.
                // TODO: deal with start of file
                todo!()
            }
        }
    }

    log!("\nfinal lo: {}", lo);
    log!(
        "made {} jumps vs. expected log(n) {}",
        jumps,
        (seeker.approx_count() as f64).log2().ceil() as usize
    );

    let final_pos = seeker.find_record_after(lo)?.unwrap().0;

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
