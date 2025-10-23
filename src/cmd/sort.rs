use std::cmp;
use std::io::{stdout, Write};
use std::iter::once;
use std::path::Path;
use std::str;

use bytesize::MB;
use colored::Colorize;
use ext_sort::{buffer::mem::MemoryLimitedBufferBuilder, ExternalSorter, ExternalSorterBuilder};
use rayon::slice::ParallelSliceMut;
use simd_csv::{ByteRecord, Error};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::select::{SelectColumns, Selection};
use crate::util::{self, DeepSizedByteRecord};
use crate::CliResult;

use self::Number::{Float, Int};

macro_rules! sort_by {
    ($target:ident, $fn:ident, $sel:ident, $numeric:ident, $reverse:ident) => {
        match ($numeric, $reverse) {
            (false, false) => $target.$fn(|r1, r2| {
                let a = $sel.select(r1);
                let b = $sel.select(r2);
                iter_cmp(a, b)
            }),
            (true, false) => $target.$fn(|r1, r2| {
                let a = $sel.select(r1);
                let b = $sel.select(r2);
                iter_cmp_num(a, b)
            }),
            (false, true) => $target.$fn(|r1, r2| {
                let a = $sel.select(r1);
                let b = $sel.select(r2);
                iter_cmp(b, a)
            }),
            (true, true) => $target.$fn(|r1, r2| {
                let a = $sel.select(r1);
                let b = $sel.select(r2);
                iter_cmp_num(b, a)
            }),
        }
    };
}

static USAGE: &str = "
Sort CSV data.

This requires reading all of the data into memory, unless
using the -e/--external flag, which will be slower and fallback
to using disk space.

Usage:
    xan sort [options] [<input>]

sort options:
    --check                   Verify whether the file is already sorted.
    -s, --select <arg>        Select a subset of columns to sort.
                              See 'xan select --help' for the format details.
    -N, --numeric             Compare according to string numerical value
    -R, --reverse             Reverse order
    -c, --count <name>        Number of times the line was consecutively duplicated.
                              Needs a column name. Can only be used with --uniq.
    -u, --uniq                When set, identical consecutive lines will be dropped
                              to keep only one line per sorted value.
    -U, --unstable            Unstable sort. Can improve performance. Does not work
                              with -e/--external.
    -p, --parallel            Whether to use parallelism to improve performance.
    -e, --external            Whether to use external sorting if you cannot fit the
                              whole file in memory.
    --tmp-dir <arg>           Directory where external sorting chunks will be written.
                              Will default to the sorted file's directory or \"./\" if
                              sorting an incoming stream.
    -m, --memory-limit <arg>  Maximum allowed memory when using external sorting, in
                              megabytes. [default: 512].
    -C, --cells               Sort the selected cell values instead of the file itself,
                              without re-ordering the columns. Runs in constant memory,
                              can be streamed and can be used to e.g. make sure an
                              edgelist always has the source & target keys in a consistent
                              order.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Namely, it will be sorted with the rest
                           of the rows. Otherwise, the first row will always
                           appear as the header row in the output.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_check: bool,
    flag_select: SelectColumns,
    flag_numeric: bool,
    flag_reverse: bool,
    flag_count: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_uniq: bool,
    flag_unstable: bool,
    flag_parallel: bool,
    flag_external: bool,
    flag_tmp_dir: Option<String>,
    flag_memory_limit: u64,
    flag_cells: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let numeric = args.flag_numeric;
    let reverse = args.flag_reverse;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);
    let count = &args.flag_count;

    if !count.is_none() && !args.flag_uniq {
        Err("-c/--count can only be used with -u/--uniq!")?;
    };

    if args.flag_unstable && args.flag_external {
        Err("-U/--unstable cannot be used with -e/--external!")?;
    }

    let mut rdr = rconfig.simd_reader()?;

    let mut headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    // Checking order
    if args.flag_check {
        let mut record = ByteRecord::new();

        let mut last: Option<Vec<Vec<u8>>> = None;

        let mut count: u64 = 0;

        while rdr.read_byte_record(&mut record)? {
            let current_sel = sel
                .select(&record)
                .map(|part| part.to_vec())
                .collect::<Vec<_>>();

            match last {
                None => {
                    last = Some(current_sel);
                }
                Some(ref last_sel) => {
                    let ordering = match (args.flag_reverse, args.flag_numeric) {
                        (false, false) => iter_cmp(current_sel.iter(), last_sel.iter()),
                        (true, false) => iter_cmp(last_sel.iter(), current_sel.iter()),
                        (false, true) => iter_cmp_num(
                            current_sel.iter().map(|r| r.as_slice()),
                            last_sel.iter().map(|r| r.as_slice()),
                        ),
                        (true, true) => iter_cmp_num(
                            last_sel.iter().map(|r| r.as_slice()),
                            current_sel.iter().map(|r| r.as_slice()),
                        ),
                    };

                    match ordering {
                        cmp::Ordering::Less => {
                            let max_len_of_head_sel = sel
                                .select(&headers)
                                .map(|h| str::from_utf8(h).unwrap().width())
                                .max()
                                .unwrap();

                            let max_len_of_last_sel = last_sel
                                .iter()
                                .map(|c| str::from_utf8(c).unwrap().width())
                                .max()
                                .unwrap();

                            let msg = format!(
                                "file is NOT sorted!\n{}Previous record (index {}) Diverging record (index {})\n{}",
                                " ".repeat(max_len_of_head_sel+1),
                                count - 1,
                                count,
                                sel.select(&headers).map(|h| {
                                    let head_to_print = str::from_utf8(h).unwrap();
                                    format!("{}{}", head_to_print, " ".repeat(max_len_of_head_sel - head_to_print.width()))
                                })
                                .zip(last_sel
                                        .iter()
                                        .map(|cell| format!("{}{}", str::from_utf8(cell).unwrap(), " ".repeat(max_len_of_last_sel - str::from_utf8(cell).unwrap().width())))
                                        .zip(current_sel
                                                .iter()
                                                .zip(last_sel)
                                                .map(|(c,l)| if (match (args.flag_reverse, args.flag_numeric) {
                                                    (false, false) => iter_cmp(once(c), once(l)),
                                                    (true, false) => iter_cmp(once(l), once(c)),
                                                    (false, true) => iter_cmp_num(once(c.as_slice()), once(l.as_slice())),
                                                    (true, true) => iter_cmp_num(once(l.as_slice()), once(c.as_slice())),
                                                }) == cmp::Ordering::Less {
                                                    str::from_utf8(c).unwrap().red().bold().to_string()
                                                } else {
                                                    str::from_utf8(c).unwrap().green().to_string()
                                                })
                                        )).map(|(h, (l, c))| format!("{} {} {}", h, l, c))
                                .collect::<Vec<_>>()
                                .join("\n")
                            );
                            Err(msg)?;
                        }
                        cmp::Ordering::Equal => continue,
                        _ => last = Some(current_sel),
                    }
                }
            };
            count += 1;
        }

        writeln!(&mut stdout(), "file is correctly sorted!")?;

        return Ok(());
    }

    // Sorting cells
    if args.flag_cells {
        let mut wtr = Config::new(&args.flag_output).simd_writer()?;

        if !args.flag_no_headers {
            wtr.write_byte_record(&headers)?;
        }

        let mask = sel.indexed_mask(headers.len());

        let mut record = ByteRecord::new();
        let mut output_record = ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            let cells: Vec<&[u8]> = if args.flag_numeric {
                let mut argsort = sel
                    .select(&record)
                    .map(|cell| {
                        parse_num(cell)
                            .map(|number| (cell, number))
                            .ok_or("Could not parse cell as number!")
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                if args.flag_reverse {
                    argsort.sort_by(|a, b| compare_num(b.1, a.1));
                } else {
                    argsort.sort_by(|a, b| compare_num(a.1, b.1));
                }

                argsort.into_iter().map(|(cell, _)| cell).collect()
            } else {
                let mut argsort = sel.select(&record).collect::<Vec<_>>();
                argsort.sort();

                if args.flag_reverse {
                    argsort.reverse();
                }

                argsort
            };

            output_record.clear();

            for (i, opt) in mask.iter().enumerate() {
                if let Some(j) = opt {
                    output_record.push_field(cells[*j]);
                } else {
                    output_record.push_field(&record[i]);
                }
            }

            wtr.write_byte_record(&output_record)?;
        }

        return Ok(wtr.flush()?);
    }

    // Sorting rows
    let all: Box<dyn Iterator<Item = ByteRecord>> = if args.flag_external {
        let tmp_dir = args.flag_tmp_dir.unwrap_or(match args.arg_input {
            None => "./".to_string(),
            Some(p) => Path::new(&p)
                .parent()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        });

        let mut sorter_builder = ExternalSorterBuilder::new()
            .with_tmp_dir(Path::new(&tmp_dir))
            .with_buffer(MemoryLimitedBufferBuilder::new(args.flag_memory_limit * MB));

        if args.flag_parallel {
            sorter_builder = sorter_builder.with_threads_number(num_cpus::get());
        }

        let sorter: ExternalSorter<
            DeepSizedByteRecord,
            Error,
            MemoryLimitedBufferBuilder,
            util::CsvExternalChunk,
        > = sorter_builder.build().unwrap();

        let sorted = sorter
            .sort_by(
                rdr.byte_records()
                    .map(|result| result.map(DeepSizedByteRecord)),
                |r1, r2| {
                    let a = sel.select(r1.as_ref());
                    let b = sel.select(r2.as_ref());

                    match (numeric, reverse) {
                        (false, false) => iter_cmp(a, b),
                        (true, false) => iter_cmp_num(a, b),
                        (false, true) => iter_cmp(b, a),
                        (true, true) => iter_cmp_num(b, a),
                    }
                },
            )
            .unwrap()
            .map(|result| result.unwrap().into_inner());

        Box::new(sorted)
    } else {
        let mut all = rdr.byte_records().collect::<Result<Vec<_>, _>>()?;

        if args.flag_unstable {
            if args.flag_parallel {
                sort_by!(all, par_sort_unstable_by, sel, numeric, reverse);
            } else {
                sort_by!(all, sort_unstable_by, sel, numeric, reverse);
            }
        } else if args.flag_parallel {
            sort_by!(all, par_sort_by, sel, numeric, reverse);
        } else {
            sort_by!(all, sort_by, sel, numeric, reverse);
        }

        Box::new(all.into_iter())
    };

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconfig.no_headers {
        if let Some(count_name) = count {
            headers.push_field(count_name.as_bytes());
        }
        if !headers.is_empty() {
            wtr.write_record(&headers)?;
        }
    }

    let mut prev: Option<ByteRecord> = None;
    let mut counter: u64 = 1;
    let mut line_buffer: Option<ByteRecord> = None;

    for r in all.into_iter() {
        if args.flag_uniq {
            match prev {
                Some(other_r) => match iter_cmp(sel.select(&r), sel.select(&other_r)) {
                    cmp::Ordering::Equal => {
                        if !count.is_none() {
                            counter += 1;
                        }
                    }
                    _ => {
                        if let Some(mut to_flush) = line_buffer {
                            to_flush.push_field(counter.to_string().as_bytes());
                            wtr.write_byte_record(&to_flush)?;
                            line_buffer = Some(r.clone());
                            counter = 1;
                        } else {
                            wtr.write_byte_record(&r)?;
                        }
                    }
                },
                None => {
                    if !count.is_none() {
                        line_buffer = Some(r.clone());
                    } else {
                        wtr.write_byte_record(&r)?;
                    }
                }
            }

            prev = Some(r);
        } else {
            wtr.write_byte_record(&r)?;
        }
    }
    if let Some(mut to_flush) = line_buffer {
        to_flush.push_field(counter.to_string().as_bytes());
        wtr.write_byte_record(&to_flush)?;
    }
    Ok(wtr.flush()?)
}

/// Order `a` and `b` lexicographically using `Ord`
pub fn iter_cmp<A, L, R>(mut a: L, mut b: R) -> cmp::Ordering
where
    A: Ord,
    L: Iterator<Item = A>,
    R: Iterator<Item = A>,
{
    loop {
        match (a.next(), b.next()) {
            (None, None) => return cmp::Ordering::Equal,
            (None, _) => return cmp::Ordering::Less,
            (_, None) => return cmp::Ordering::Greater,
            (Some(x), Some(y)) => match x.cmp(&y) {
                cmp::Ordering::Equal => (),
                non_eq => return non_eq,
            },
        }
    }
}

/// Try parsing `a` and `b` as numbers when ordering
pub fn iter_cmp_num<'a, L, R>(mut a: L, mut b: R) -> cmp::Ordering
where
    L: Iterator<Item = &'a [u8]>,
    R: Iterator<Item = &'a [u8]>,
{
    loop {
        match (next_num(&mut a), next_num(&mut b)) {
            (None, None) => return cmp::Ordering::Equal,
            (None, _) => return cmp::Ordering::Less,
            (_, None) => return cmp::Ordering::Greater,
            (Some(x), Some(y)) => match compare_num(x, y) {
                cmp::Ordering::Equal => (),
                non_eq => return non_eq,
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Number {
    Int(i64),
    Float(f64),
}

fn compare_num(n1: Number, n2: Number) -> cmp::Ordering {
    match (n1, n2) {
        (Int(i1), Int(i2)) => i1.cmp(&i2),
        (Int(i1), Float(f2)) => compare_float(i1 as f64, f2),
        (Float(f1), Int(i2)) => compare_float(f1, i2 as f64),
        (Float(f1), Float(f2)) => compare_float(f1, f2),
    }
}

fn compare_float(f1: f64, f2: f64) -> cmp::Ordering {
    f1.partial_cmp(&f2).unwrap_or(cmp::Ordering::Equal)
}

fn parse_num(bytes: &[u8]) -> Option<Number> {
    if let Ok(i) = btoi::btoi::<i64>(bytes) {
        Some(Number::Int(i))
    } else if let Ok(f) = fast_float::parse(bytes) {
        Some(Number::Float(f))
    } else {
        None
    }
}

fn next_num<'a, X>(xs: &mut X) -> Option<Number>
where
    X: Iterator<Item = &'a [u8]>,
{
    xs.next().and_then(parse_num)
}

// Standard comparable byte record abstraction
pub struct ComparableByteRecord<'a> {
    record: ByteRecord,
    sel: &'a Selection,
}

impl<'a> ComparableByteRecord<'a> {
    pub fn new(record: ByteRecord, sel: &'a Selection) -> Self {
        ComparableByteRecord { record, sel }
    }

    pub fn as_byte_record(&self) -> &ByteRecord {
        &self.record
    }
}

impl cmp::Ord for ComparableByteRecord<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        let s1 = self.sel.select(&self.record);
        let s2 = other.sel.select(&other.record);

        iter_cmp(s1, s2)
    }
}

impl cmp::PartialOrd for ComparableByteRecord<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::PartialEq for ComparableByteRecord<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl cmp::Eq for ComparableByteRecord<'_> {}

// Numerically comparable byte record abstraction
pub struct NumericallyComparableByteRecord<'a> {
    record: ByteRecord,
    sel: &'a Selection,
}

impl<'a> NumericallyComparableByteRecord<'a> {
    pub fn new(record: ByteRecord, sel: &'a Selection) -> Self {
        NumericallyComparableByteRecord { record, sel }
    }

    pub fn as_byte_record(&self) -> &ByteRecord {
        &self.record
    }
}

impl cmp::Ord for NumericallyComparableByteRecord<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        let s1 = self.sel.select(&self.record);
        let s2 = other.sel.select(&other.record);

        iter_cmp_num(s1, s2)
    }
}

impl cmp::PartialOrd for NumericallyComparableByteRecord<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::PartialEq for NumericallyComparableByteRecord<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl cmp::Eq for NumericallyComparableByteRecord<'_> {}
