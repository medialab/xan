use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use colored::Colorize;

use crate::cmd::sort::{ComparableByteRecord, NumericallyComparableByteRecord};
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Merge multiple CSV files already sorted the same way. Those files MUST:

1. have the same columns in the same order.
2. have the same row order wrt -s/--select, -R/--reverse & -N/--numeric

If those conditions are not met, the result will be in arbitrary order.

This command consumes memory proportional to one CSV row per file.

Usage:
    xan merge [options] [<input>...]
    xan merge --help

merge options:
    -s, --select <arg>          Select a subset of columns to sort.
                                See 'xan select --help' for the format details.
    -N, --numeric               Compare according to string numerical value
    -R, --reverse               Reverse order
    -u, --uniq                  When set, identical consecutive lines will be dropped
                                to keep only one line per sorted value.
    -S, --source-column <name>  Name of a column to prepend in the output of the command
                                indicating the path to source file.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(PartialEq, PartialOrd, Ord, Eq)]
struct Forward<T>(T);

#[derive(Deserialize)]
struct Args {
    arg_input: Vec<String>,
    flag_select: SelectColumns,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_numeric: bool,
    flag_reverse: bool,
    flag_uniq: bool,
    flag_source_column: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let confs = args.configs()?.into_iter().collect::<Vec<Config>>();
    let paths = confs
        .iter()
        .map(|c| {
            c.path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or("<stdin>".to_string())
        })
        .collect::<Vec<_>>();

    let mut readers = confs
        .iter()
        .map(|conf| conf.reader())
        .collect::<Result<Vec<_>, _>>()?;

    let headers = readers
        .iter_mut()
        .map(|rdr| rdr.byte_headers())
        .collect::<Result<Vec<_>, _>>()?;

    if !args.flag_no_headers {
        if let Some(i) = headers.iter().skip(1).position(|h| *h != headers[0]) {
            let path = &paths[i + 1];

            Err(format!("All given files should have identical headers, and in the same order!\nFirst diverging file: {}", path.cyan()))?;
        }

        if let Some(name) = &args.flag_source_column {
            wtr.write_record([name.as_bytes()].into_iter().chain(headers[0]))?;
        } else {
            wtr.write_byte_record(headers[0])?;
        }
    }

    let selections = confs
        .iter()
        .zip(headers.iter())
        .map(|(c, h)| c.selection(h))
        .collect::<Result<Vec<_>, _>>()?;

    let mut record_iterators = readers
        .into_iter()
        .map(|rdr| rdr.into_byte_records())
        .collect::<Vec<_>>();

    macro_rules! write_record {
        ($i:ident, $record:expr) => {
            if args.flag_source_column.is_some() {
                wtr.write_record([paths[$i].as_bytes()].into_iter().chain($record))
            } else {
                wtr.write_byte_record($record)
            }
        };
    }

    macro_rules! kway {
        ($wrapper:ident, $record:ident) => {
            let mut heap: BinaryHeap<($wrapper<$record>, usize)> =
                BinaryHeap::with_capacity(record_iterators.len());

            for (i, (iter, sel)) in record_iterators
                .iter_mut()
                .zip(selections.iter())
                .enumerate()
            {
                match iter.next() {
                    None => continue,
                    Some(record) => {
                        let record = $wrapper($record::new(record?, sel));
                        heap.push((record, i));
                    }
                }
            }

            let mut last_record: Option<$wrapper<$record>> = None;

            while !heap.is_empty() {
                match heap.pop() {
                    None => break,
                    Some(entry) => {
                        let (comparable_record, i) = entry;

                        if args.flag_uniq {
                            match last_record {
                                None => {
                                    write_record!(i, comparable_record.0.as_byte_record())?;
                                    last_record = Some(comparable_record);
                                }
                                Some(ref r) => match r.cmp(&comparable_record) {
                                    Ordering::Equal => (),
                                    _ => {
                                        write_record!(i, comparable_record.0.as_byte_record())?;
                                        last_record = Some(comparable_record);
                                    }
                                },
                            }
                        } else {
                            write_record!(i, comparable_record.0.as_byte_record())?;
                        }

                        match record_iterators[i].next() {
                            None => continue,
                            Some(record) => {
                                let record = $wrapper($record::new(record?, &selections[i]));
                                heap.push((record, i));
                            }
                        }
                    }
                }
            }
        };
    }

    match (args.flag_numeric, args.flag_reverse) {
        (false, false) => {
            kway!(Reverse, ComparableByteRecord);
        }
        (true, false) => {
            kway!(Reverse, NumericallyComparableByteRecord);
        }
        (false, true) => {
            kway!(Forward, ComparableByteRecord);
        }
        (true, true) => {
            kway!(Forward, NumericallyComparableByteRecord);
        }
    };

    Ok(wtr.flush()?)
}

impl Args {
    fn configs(&self) -> CliResult<Vec<Config>> {
        util::many_configs(
            &self.arg_input,
            self.flag_delimiter,
            self.flag_no_headers,
            Some(&self.flag_select),
        )
        .map_err(From::from)
    }
}
