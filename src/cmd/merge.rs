use std::cmp::Ordering;
use std::io::Write;

use binary_heap_plus::BinaryHeap;
use colored::Colorize;
use simd_csv::{ByteRecord, Writer};

use crate::cmd::sort::{iter_cmp, iter_cmp_num};
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Merge multiple CSV files already sorted the same way. Those files MUST:

1. have the same columns in the same order.
2. have the same row order wrt -s/--select, -R/--reverse & -N/--numeric

If those conditions are not met, the result will be in arbitrary order.

This command consumes memory proportional to one CSV row per file.

When merging a large number of CSV files exceeding your shell's
command arguments limit, prefer using the --paths flag to read the list of CSV
files to merge from input lines or from a CSV file containing paths in a
column given to the --path-column flag.

Note that all the files will need to be opened at once, so you might hit the
maximum number of opened files of your OS then.

Feeding --paths lines:

    $ xan merge --paths paths.txt > merged.csv

Feeding --paths CSV file:

    $ xan merge --paths files.csv --path-column path > merged.csv

Feeding stdin (\"-\") to --paths:

    $ find . -name '*.csv' | xan merge --paths - > merged.csv

Feeding CSV as stdin (\"-\") to --paths:

    $ cat filelist.csv | xan merge --paths - --path-column path > merged.csv

Usage:
    xan merge [options] [<inputs>...]
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
    --paths <input>             Give a text file (use \"-\" for stdin) containing one path of
                                CSV file to concatenate per line, instead of giving the paths
                                through the command's arguments.
    --path-column <name>        When given a column name, --paths will be considered as CSV, and paths
                                to CSV files to merge will be extracted from the selected column.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_inputs: Vec<String>,
    flag_select: SelectedColumns,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_numeric: bool,
    flag_reverse: bool,
    flag_uniq: bool,
    flag_source_column: Option<String>,
    flag_paths: Option<String>,
    flag_path_column: Option<SelectedColumns>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_paths.is_some() && !args.arg_inputs.is_empty() {
        Err("--paths cannot be used with other positional arguments!")?;
    }

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

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
        .map(|conf| conf.simd_reader())
        .collect::<Result<Vec<_>, _>>()?;

    let headers = readers
        .iter_mut()
        .map(|rdr| rdr.byte_headers())
        .collect::<Result<Vec<_>, _>>()?;

    if !confs[0].no_headers {
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

    let sel = confs
        .iter()
        .zip(headers.iter())
        .map(|(c, h)| c.selection(*h))
        .next()
        .unwrap()?;

    let mut record_iterators = readers
        .into_iter()
        .map(|rdr| rdr.into_byte_records())
        .collect::<Vec<_>>();

    fn write_record<W: Write>(
        writer: &mut Writer<W>,
        record: &ByteRecord,
        path_opt: Option<&[u8]>,
    ) -> simd_csv::Result<()> {
        if let Some(path) = path_opt {
            writer.write_record([path].into_iter().chain(record))
        } else {
            writer.write_byte_record(record)
        }
    }

    let cmp_record = |a: &ByteRecord, b: &ByteRecord| -> Ordering {
        let a_sel = sel.select(a);
        let b_sel = sel.select(b);

        let ordering = if args.flag_numeric {
            iter_cmp_num(a_sel, b_sel)
        } else {
            iter_cmp(a_sel, b_sel)
        };

        // NOTE: remember the heap is a max heap
        if args.flag_reverse {
            ordering
        } else {
            ordering.reverse()
        }
    };

    let cmp = |a: &(ByteRecord, usize), b: &(ByteRecord, usize)| -> Ordering {
        let ordering = cmp_record(&a.0, &b.0);

        if ordering.is_eq() {
            b.1.cmp(&a.1)
        } else {
            ordering
        }
    };

    let mut heap = BinaryHeap::with_capacity_by(record_iterators.len(), cmp);

    for (i, iter) in record_iterators.iter_mut().enumerate() {
        match iter.next() {
            None => continue,
            Some(record) => {
                heap.push((record?, i));
            }
        }
    }

    let mut last_record: Option<(ByteRecord, usize)> = None;

    // TODO: factorize into a MergHeap

    while let Some(entry) = heap.pop() {
        let (record, i) = entry;

        if args.flag_uniq {
            match last_record {
                None => {
                    write_record(
                        &mut wtr,
                        &record,
                        args.flag_source_column
                            .as_ref()
                            .map(|_| paths[i].as_bytes()),
                    )?;
                    last_record = Some((record, i));
                }
                Some(ref r) => match cmp_record(&r.0, &record) {
                    Ordering::Equal => (),
                    _ => {
                        write_record(
                            &mut wtr,
                            &record,
                            args.flag_source_column
                                .as_ref()
                                .map(|_| paths[i].as_bytes()),
                        )?;
                        last_record = Some((record, i));
                    }
                },
            }
        } else {
            write_record(
                &mut wtr,
                &record,
                args.flag_source_column
                    .as_ref()
                    .map(|_| paths[i].as_bytes()),
            )?;
        }

        match record_iterators[i].next() {
            None => continue,
            Some(record) => {
                heap.push((record?, i));
            }
        }
    }

    Ok(wtr.flush()?)
}

impl Args {
    fn configs(&self) -> CliResult<Vec<Config>> {
        if let Some(path) = &self.flag_paths {
            return Config::new(&Some(path.clone()))
                .lines(&self.flag_path_column)?
                .map(|result| -> CliResult<Config> {
                    let path = result?;

                    Ok(Config::new(&Some(path))
                        .delimiter(self.flag_delimiter)
                        .no_headers(self.flag_no_headers)
                        .select(self.flag_select.clone()))
                })
                .collect::<Result<Vec<_>, _>>();
        }

        util::many_configs(
            &self.arg_inputs,
            self.flag_delimiter,
            self.flag_no_headers,
            Some(&self.flag_select),
        )
        .map_err(From::from)
    }
}
