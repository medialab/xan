use std::{cmp::Reverse, collections::HashMap};

use csv::{self, ByteRecord};
use rayon::prelude::*;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::structures::FixedReverseHeap;
use crate::util;
use crate::CliResult;

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
check the `xsv groupby` command instead.

Usage:
    xan frequency [options] [<input>]

frequency options:
    -s, --select <arg>     Select a subset of columns to compute frequencies
                           for. See 'xan select --help' for the format
                           details. This is provided here because piping 'xan
                           select' into 'xan frequency' will disable the use
                           of indexing.
    -l, --limit <arg>      Limit the frequency table to the N most common
                           items. Set to <=0 to disable a limit. It is combined
                           with -t/--threshold.
                           [default: 10]
    -t, --threshold <arg>  If set, won't return items having a count less than
                           this given threshold. It is combined with -l/--limit.
    -N, --no-extra         Don't include empty cells & remaining counts.
    -p, --parallel         Allow sorting to be done in parallel. This is only
                           useful with -l/--limit set to 0, i.e. no limit.

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
    flag_limit: usize,
    flag_threshold: Option<u64>,
    flag_no_extra: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let sel = rconf.selection(&headers)?;

    // Nothing was selected
    if sel.len() == 0 {
        return Ok(());
    }

    let mut fields: Vec<HashMap<Vec<u8>, u64>> = (0..sel.len()).map(|_| HashMap::new()).collect();

    let output_headers = {
        let mut r = ByteRecord::new();
        r.push_field(b"field");
        r.push_field(b"value");
        r.push_field(b"count");
        r
    };

    wtr.write_byte_record(&output_headers)?;

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        for (mut cell, counter) in sel.select(&record).zip(fields.iter_mut()) {
            if !args.flag_no_extra {
                if cell.is_empty() {
                    cell = b"<empty>";
                }
            } else if cell.is_empty() {
                continue;
            }

            counter
                .entry(cell.to_vec())
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    let field_names: Vec<Vec<u8>> = if args.flag_no_headers {
        sel.indices()
            .map(|i| i.to_string().as_bytes().to_vec())
            .collect()
    } else {
        sel.select(&headers).map(|h| h.to_vec()).collect()
    };

    for (name, counter) in field_names.into_iter().zip(fields.into_iter()) {
        let mut total: u64 = 0;

        // NOTE: if the limit is less than half of the dataset, we fallback to a heap
        let items = if args.flag_limit != 0
            && args.flag_limit < (counter.len() as f64 / 2.0).floor() as usize
        {
            let mut heap: FixedReverseHeap<(u64, Reverse<Vec<u8>>)> =
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
                items.sort_unstable_by(|a, b| a.1.cmp(&b.1).reverse().then_with(|| a.0.cmp(&b.0)));
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

    Ok(wtr.flush()?)
}
