use std::num::NonZeroUsize;

use bstr::ByteSlice;
use csv::{self, ByteRecord};

use crate::cmd::parallel::Args as ParallelArgs;
use crate::collections::{ClusteredInsertHashmap, Counter};
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

type GroupKey = Vec<Vec<u8>>;
type ValueKey = Vec<u8>;

static USAGE: &str = "
Compute a frequency table on CSV data.

The resulting frequency table will look like this:

field - Name of the column
value - Some distinct value of the column
count - Number of rows containing this value

By default, there is a row for the N most frequent values for each field in the
data. The number of returned values can be tweaked with -l/--limit or you can
disable the limit altogether using the -A/--all flag.

Since this computes an exact frequency table, memory proportional to the
cardinality of each selected column is required. If you expect this will overflow
your memory, you can compute an approximate top-k using the -a, --approx flag.

To compute custom aggregations per group, beyond just counting, please be sure to
check the `xan groupby` command instead.

Frequency tables can be computed in parallel using the -p/--parallel or -t/--threads
flags. This cannot work on streams or gzipped files, unless a `.gzi` index (as
created by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -g/--groupby option.

Usage:
    xan frequency [options] [<input>]
    xan freq [options] [<input>]

frequency options:
    -s, --select <arg>       Select a subset of columns to compute frequencies
                             for. See 'xan select --help' for the selection language
                             details.
    --sep <char>             Split the cell into multiple values to count using the
                             provided separator.
    -g, --groupby <cols>     If given, will compute frequency tables per group
                             as defined by the given columns.
    -A, --all                Remove the limit.
    -l, --limit <arg>        Limit the frequency table to the N most common
                             items. Use -A, -all or set to 0 to disable the limit.
                             [default: 10]
    -a, --approx             If set, return the items most likely having the top counts,
                             as per given --limit. Won't work if --limit is 0 or
                             with -A, --all. Accuracy of results increases with the
                             given limit.
    -N, --no-extra           Don't include empty cells & remaining counts.
    -p, --parallel           Whether to use parallelization to speed up computation.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

Hidden options:
    --no-limit-we-reach-for-the-sky  Nothing to see here...

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 0-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Clone, Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_sep: Option<String>,
    flag_all: bool,
    flag_limit: usize,
    flag_approx: bool,
    flag_no_extra: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<NonZeroUsize>,
    flag_groupby: Option<SelectColumns>,
    flag_no_limit_we_reach_for_the_sky: bool,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_all {
            self.flag_limit = 0;
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_groupby.is_some() {
            Err("-p/--parallel or -t/--threads cannot be used with -g/--groupby!")?;
        }

        let mut parallel_args = ParallelArgs::single_file(&args.arg_input, args.flag_threads)?;

        parallel_args.cmd_freq = true;
        parallel_args.flag_select = args.flag_select;
        parallel_args.flag_all = args.flag_all;
        parallel_args.flag_limit = args.flag_limit;
        parallel_args.flag_approx = args.flag_approx;
        parallel_args.flag_no_extra = args.flag_no_extra;

        parallel_args.flag_no_headers = args.flag_no_headers;
        parallel_args.flag_output = args.flag_output;
        parallel_args.flag_delimiter = args.flag_delimiter;

        return parallel_args.run();
    }

    args.resolve();

    if args.flag_approx && args.flag_limit == 0 {
        Err("-a, --approx cannot work with --limit=0 or -A, --all!")?;
    }

    if args.flag_no_limit_we_reach_for_the_sky {
        opener::open_browser("https://www.youtube.com/watch?v=7kmEEkECFQw")
            .expect("could not easter egg");
        return Ok(());
    }

    let approx_k = if args.flag_approx {
        Some(args.flag_limit)
    } else {
        None
    };

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let mut sel = rconf.selection(&headers)?;
    let groupby_sel_opt = args
        .flag_groupby
        .map(|cols| cols.selection(&headers, !args.flag_no_headers))
        .transpose()?;

    // No need to consider the grouping column when counting frequencies
    if let Some(gsel) = &groupby_sel_opt {
        sel.subtract(gsel);
    }

    // Nothing was selected
    if sel.is_empty() {
        return Ok(());
    }

    let field_names: Vec<Vec<u8>> = if args.flag_no_headers {
        sel.iter()
            .map(|i| i.to_string().as_bytes().to_vec())
            .collect()
    } else {
        sel.select(&headers).map(|h| h.to_vec()).collect()
    };

    fn coerce_cell(cell: &[u8], no_extra: bool) -> Option<&[u8]> {
        if !no_extra {
            if cell.is_empty() {
                Some(b"<empty>")
            } else {
                Some(cell)
            }
        } else if cell.is_empty() {
            None
        } else {
            Some(cell)
        }
    }

    if let Some(groupby_sel) = groupby_sel_opt {
        let mut groups_to_fields_to_counter: ClusteredInsertHashmap<
            GroupKey,
            Vec<Counter<ValueKey>>,
        > = ClusteredInsertHashmap::new();

        let output_headers = {
            let mut r = ByteRecord::new();
            r.push_field(b"field");

            for col_name in groupby_sel.select(&headers) {
                r.push_field(col_name);
            }

            r.push_field(b"value");
            r.push_field(b"count");
            r
        };

        wtr.write_byte_record(&output_headers)?;

        let mut record = csv::ByteRecord::new();

        // Aggregating
        while rdr.read_byte_record(&mut record)? {
            let group: Vec<_> = groupby_sel
                .select(&record)
                .map(|cell| cell.to_vec())
                .collect();

            let fields_to_counter = groups_to_fields_to_counter.insert_with(group, || {
                let mut list = Vec::with_capacity(sel.len());

                for _ in 0..sel.len() {
                    list.push(Counter::new(approx_k));
                }

                list
            });

            for (i, cell) in sel.select(&record).enumerate() {
                if let Some(sep) = &args.flag_sep {
                    for sub_cell in cell.split_str(sep) {
                        let sub_cell = match coerce_cell(sub_cell, args.flag_no_extra) {
                            Some(c) => c,
                            None => continue,
                        };

                        fields_to_counter[i].add(sub_cell.to_vec());
                    }
                } else {
                    let cell = match coerce_cell(cell, args.flag_no_extra) {
                        Some(c) => c,
                        None => continue,
                    };

                    fields_to_counter[i].add(cell.to_vec());
                }
            }
        }

        // Writing output
        for name in field_names.into_iter().rev() {
            for (group, counters) in groups_to_fields_to_counter.iter_mut() {
                let counter = counters.pop().unwrap();

                let (total, items) = counter.into_total_and_items(
                    if args.flag_limit == 0 {
                        None
                    } else {
                        Some(args.flag_limit)
                    },
                    false,
                );

                let mut emitted: u64 = 0;

                for (value, count) in items {
                    emitted += count;

                    record.clear();
                    record.push_field(&name);

                    for cell in group {
                        record.push_field(cell);
                    }

                    record.push_field(&value);
                    record.push_field(count.to_string().as_bytes());
                    wtr.write_byte_record(&record)?;
                }

                let remaining = total - emitted;

                if !args.flag_no_extra && remaining > 0 {
                    record.clear();
                    record.push_field(&name);

                    for cell in group {
                        record.push_field(cell);
                    }

                    record.push_field(b"<rest>");
                    record.push_field(remaining.to_string().as_bytes());
                    wtr.write_byte_record(&record)?;
                }
            }
        }
    } else {
        let mut fields: Vec<Counter<ValueKey>> =
            (0..sel.len()).map(|_| Counter::new(approx_k)).collect();

        let output_headers = {
            let mut r = ByteRecord::new();
            r.push_field(b"field");
            r.push_field(b"value");
            r.push_field(b"count");
            r
        };

        wtr.write_byte_record(&output_headers)?;

        let mut record = csv::ByteRecord::new();

        // Aggregating
        while rdr.read_byte_record(&mut record)? {
            for (cell, counter) in sel.select(&record).zip(fields.iter_mut()) {
                if let Some(sep) = &args.flag_sep {
                    for sub_cell in cell.split_str(sep) {
                        let sub_cell = match coerce_cell(sub_cell, args.flag_no_extra) {
                            Some(c) => c,
                            None => continue,
                        };

                        counter.add(sub_cell.to_vec());
                    }
                } else {
                    let cell = match coerce_cell(cell, args.flag_no_extra) {
                        Some(c) => c,
                        None => continue,
                    };

                    counter.add(cell.to_vec());
                }
            }
        }

        // Writing output
        for (name, counter) in field_names.into_iter().zip(fields.into_iter()) {
            let (total, items) = counter.into_total_and_items(
                if args.flag_limit == 0 {
                    None
                } else {
                    Some(args.flag_limit)
                },
                false,
            );

            let mut emitted: u64 = 0;

            for (value, count) in items {
                emitted += count;

                record.clear();
                record.push_field(&name);
                record.push_field(&value);
                record.push_field(count.to_string().as_bytes());
                wtr.write_byte_record(&record)?;
            }

            let remaining = total - emitted;

            if !args.flag_no_extra && remaining > 0 {
                record.clear();
                record.push_field(&name);
                record.push_field(b"<rest>");
                record.push_field(remaining.to_string().as_bytes());
                wtr.write_byte_record(&record)?;
            }
        }
    }

    Ok(wtr.flush()?)
}
