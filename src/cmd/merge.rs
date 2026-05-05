use std::cmp::Ordering;

use binary_heap_plus::BinaryHeap;
use colored::Colorize;
use compare::Compare;
use simd_csv::ByteRecord;

use crate::cmd::sort::{iter_cmp, iter_cmp_num};
use crate::config::{Config, Delimiter};
use crate::select::{SelectedColumns, Selection};
use crate::util;
use crate::CliResult;

type MergeHeapEntry = (ByteRecord, usize);

#[derive(Clone)]
pub struct MergeHeapComparator<'s> {
    sel: &'s Selection,
    numeric: bool,
    reverse: bool,
}

impl<'s> MergeHeapComparator<'s> {
    pub fn new(sel: &'s Selection, numeric: bool, reverse: bool) -> Self {
        Self {
            sel,
            numeric,
            reverse,
        }
    }

    fn cmp_record(&self, a: &ByteRecord, b: &ByteRecord) -> Ordering {
        let a_sel = self.sel.select(a);
        let b_sel = self.sel.select(b);

        let ordering = if self.numeric {
            iter_cmp_num(a_sel, b_sel)
        } else {
            iter_cmp(a_sel, b_sel)
        };

        // NOTE: remember the heap is a max heap
        if self.reverse {
            ordering
        } else {
            ordering.reverse()
        }
    }

    #[inline]
    fn cmp(&self, a: &MergeHeapEntry, b: &MergeHeapEntry) -> Ordering {
        let ordering = self.cmp_record(&a.0, &b.0);

        if ordering.is_eq() {
            b.1.cmp(&a.1)
        } else {
            ordering
        }
    }
}

impl Compare<MergeHeapEntry> for MergeHeapComparator<'_> {
    #[inline(always)]
    fn compare(&self, l: &MergeHeapEntry, r: &MergeHeapEntry) -> Ordering {
        self.cmp(l, r)
    }
}

pub struct MergeHeap<'s> {
    inner: BinaryHeap<MergeHeapEntry, MergeHeapComparator<'s>>,
}

impl<'s> MergeHeap<'s> {
    pub fn with_comparator(comparator: MergeHeapComparator<'s>) -> Self {
        let inner = BinaryHeap::from_vec_cmp(Vec::with_capacity(comparator.sel.len()), comparator);

        Self { inner }
    }

    #[inline(always)]
    fn push(&mut self, entry: MergeHeapEntry) {
        self.inner.push(entry);
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<MergeHeapEntry> {
        self.inner.pop()
    }

    pub fn into_iter<I>(self, iterators: Vec<I>) -> simd_csv::Result<MergeHeapIntoIter<'s, I>>
    where
        I: Iterator<Item = simd_csv::Result<ByteRecord>>,
    {
        MergeHeapIntoIter::new(self, iterators)
    }
}

pub struct MergeHeapIntoIter<'s, I> {
    heap: MergeHeap<'s>,
    iterators: Vec<I>,
}

impl<'s, I> MergeHeapIntoIter<'s, I>
where
    I: Iterator<Item = simd_csv::Result<ByteRecord>>,
{
    fn new(mut heap: MergeHeap<'s>, mut iterators: Vec<I>) -> simd_csv::Result<Self> {
        for (i, iter) in iterators.iter_mut().enumerate() {
            match iter.next() {
                None => continue,
                Some(record) => {
                    heap.push((record?, i));
                }
            }
        }

        Ok(Self { heap, iterators })
    }
}

impl<I> Iterator for MergeHeapIntoIter<'_, I>
where
    I: Iterator<Item = simd_csv::Result<ByteRecord>>,
{
    type Item = simd_csv::Result<(usize, ByteRecord)>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entry) = self.heap.pop() {
            let (record, i) = entry;

            match self.iterators[i].next() {
                None => (),
                Some(result) => match result {
                    Ok(record) => {
                        self.heap.push((record, i));
                    }
                    Err(err) => {
                        return Some(Err(err));
                    }
                },
            }

            Some(Ok((i, record)))
        } else {
            None
        }
    }
}

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

    let record_iterators = readers
        .into_iter()
        .map(|rdr| rdr.into_byte_records())
        .collect::<Vec<_>>();

    let comparator = MergeHeapComparator::new(&sel, args.flag_numeric, args.flag_reverse);
    let heap = MergeHeap::with_comparator(comparator.clone());

    let mut last_record: Option<(ByteRecord, usize)> = None;

    macro_rules! write_record {
        ($i: ident, $record: ident) => {{
            if args.flag_source_column.is_some() {
                wtr.write_record([paths[$i].as_bytes()].into_iter().chain(&$record))?;
            } else {
                wtr.write_byte_record(&$record)?;
            }
        }};
    }

    for result in heap.into_iter(record_iterators)? {
        let (i, record) = result?;

        if args.flag_uniq {
            match last_record {
                None => {
                    write_record!(i, record);
                    last_record = Some((record, i));
                }
                Some(ref r) => match comparator.cmp_record(&r.0, &record) {
                    Ordering::Equal => (),
                    _ => {
                        write_record!(i, record);
                        last_record = Some((record, i));
                    }
                },
            }
        } else {
            write_record!(i, record);
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
