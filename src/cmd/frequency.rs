use std::{cmp::Reverse, collections::HashMap};

use bstr::ByteSlice;
use csv::{self, ByteRecord};
use rayon::prelude::*;

use crate::collections::{ClusteredInsertHashmap, FixedReverseHeap};
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
data. The number of values can be tweaked with --limit and --threshold flags
respectively.

Since this computes an exact frequency table, memory proportional to the
cardinality of each selected column is required.

To compute custom aggregations per group, beyond just counting, please be sure to
check the `xan groupby` command instead.

Usage:
    xan frequency [options] [<input>]
    xan freq [options] [<input>]

frequency options:
    -s, --select <arg>     Select a subset of columns to compute frequencies
                           for. See 'xan select --help' for the selection language
                           details.
    --sep <char>           Split the cell into multiple values to count using the
                           provided separator.
    -g, --groupby <cols>   If given, will compute frequency tables per group
                           as defined by the given columns.
    -l, --limit <arg>      Limit the frequency table to the N most common
                           items. Set to <=0 to disable a limit. It is combined
                           with -t/--threshold.
                           [default: 10]
    -t, --threshold <arg>  If set, won't return items having a count less than
                           this given threshold. It is combined with -l/--limit.
    -N, --no-extra         Don't include empty cells & remaining counts.
    -p, --parallel         Allow sorting to be done in parallel. This is only
                           useful with -l/--limit set to 0, i.e. no limit.

Hidden options:
    --no-limit-we-reach-for-the-sky  Nothing to see here...

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 1-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Clone, Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_sep: Option<String>,
    flag_limit: usize,
    flag_threshold: Option<u64>,
    flag_no_extra: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_groupby: Option<SelectColumns>,
    flag_no_limit_we_reach_for_the_sky: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_no_limit_we_reach_for_the_sky {
        open::that("https://www.youtube.com/watch?v=7kmEEkECFQw")?;
        return Ok(());
    }

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
        sel.indices()
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
            Vec<HashMap<ValueKey, u64>>,
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

        let mut insert = |g: &Vec<Vec<u8>>, i: usize, c: &[u8]| {
            groups_to_fields_to_counter.insert_with_or_else(
                g.clone(),
                || {
                    let mut list = Vec::with_capacity(sel.len());

                    for _ in 0..sel.len() {
                        list.push(HashMap::new());
                    }

                    list[i]
                        .entry(c.to_vec())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    list
                },
                |list| {
                    list[i]
                        .entry(c.to_vec())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                },
            );
        };

        // Aggregating
        while rdr.read_byte_record(&mut record)? {
            let group: Vec<_> = groupby_sel
                .select(&record)
                .map(|cell| cell.to_vec())
                .collect();

            for (i, cell) in sel.select(&record).enumerate() {
                if let Some(sep) = &args.flag_sep {
                    for sub_cell in cell.split_str(sep) {
                        let sub_cell = match coerce_cell(sub_cell, args.flag_no_extra) {
                            Some(c) => c,
                            None => continue,
                        };

                        insert(&group, i, sub_cell);
                    }
                } else {
                    let cell = match coerce_cell(cell, args.flag_no_extra) {
                        Some(c) => c,
                        None => continue,
                    };

                    insert(&group, i, cell);
                }
            }
        }

        // Writing output
        for (i, name) in field_names.into_iter().enumerate() {
            for (group, counters) in groups_to_fields_to_counter.iter() {
                let counter = &counters[i];

                let mut total: u64 = 0;

                // NOTE: if the limit is less than half of the dataset, we fallback to a heap
                let items = if args.flag_limit != 0
                    && args.flag_limit < (counter.len() as f64 / 2.0).floor() as usize
                {
                    let mut heap: FixedReverseHeap<(u64, Reverse<&ValueKey>)> =
                        FixedReverseHeap::with_capacity(args.flag_limit);

                    for (value, count) in counter {
                        total += count;

                        heap.push((*count, Reverse(value)));
                    }

                    heap.into_sorted_vec()
                        .into_iter()
                        .map(|(count, Reverse(value))| (value, count))
                        .collect()
                } else {
                    let mut items = counter
                        .iter()
                        .map(|(v, c)| (v, *c))
                        .inspect(|(_, c)| total += c)
                        .collect::<Vec<_>>();

                    if args.flag_parallel {
                        items.par_sort_unstable_by(|a, b| {
                            a.1.cmp(&b.1).reverse().then_with(|| a.0.cmp(b.0))
                        });
                    } else {
                        items.sort_unstable_by(|a, b| {
                            a.1.cmp(&b.1).reverse().then_with(|| a.0.cmp(b.0))
                        });
                    }

                    if args.flag_limit != 0 {
                        items.truncate(args.flag_limit);
                    }

                    items
                };

                let mut emitted: u64 = 0;

                for (value, count) in items {
                    if let Some(threshold) = args.flag_threshold {
                        if count < threshold {
                            break;
                        }
                    }

                    emitted += count;

                    record.clear();
                    record.push_field(&name);

                    for cell in group {
                        record.push_field(cell);
                    }

                    record.push_field(value);
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
        let mut fields: Vec<HashMap<ValueKey, u64>> =
            (0..sel.len()).map(|_| HashMap::new()).collect();

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

                        counter
                            .entry(sub_cell.to_vec())
                            .and_modify(|count| *count += 1)
                            .or_insert(1);
                    }
                } else {
                    let cell = match coerce_cell(cell, args.flag_no_extra) {
                        Some(c) => c,
                        None => continue,
                    };

                    counter
                        .entry(cell.to_vec())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                }
            }
        }

        // Writing output
        for (name, counter) in field_names.into_iter().zip(fields.into_iter()) {
            let mut total: u64 = 0;

            // NOTE: if the limit is less than half of the dataset, we fallback to a heap
            let items = if args.flag_limit != 0
                && args.flag_limit < (counter.len() as f64 / 2.0).floor() as usize
            {
                let mut heap: FixedReverseHeap<(u64, Reverse<ValueKey>)> =
                    FixedReverseHeap::with_capacity(args.flag_limit);

                for (value, count) in counter {
                    total += count;

                    heap.push((count, Reverse(value)));
                }

                heap.into_sorted_vec()
                    .into_iter()
                    .map(|(count, Reverse(value))| (value, count))
                    .collect()
            } else {
                let mut items = counter
                    .into_iter()
                    .inspect(|(_, c)| total += c)
                    .collect::<Vec<_>>();

                if args.flag_parallel {
                    items.par_sort_unstable_by(|a, b| {
                        a.1.cmp(&b.1).reverse().then_with(|| a.0.cmp(&b.0))
                    });
                } else {
                    items.sort_unstable_by(|a, b| {
                        a.1.cmp(&b.1).reverse().then_with(|| a.0.cmp(&b.0))
                    });
                }

                if args.flag_limit != 0 {
                    items.truncate(args.flag_limit);
                }

                items
            };

            let mut emitted: u64 = 0;

            for (value, count) in items {
                if let Some(threshold) = args.flag_threshold {
                    if count < threshold {
                        break;
                    }
                }

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
