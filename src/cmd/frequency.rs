use std::collections::HashMap;

use csv::{self, ByteRecord};

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliResult;

static USAGE: &str = "
Compute a frequency table on CSV data.

The frequency table is formatted as CSV data:

    field,value,count

By default, there is a row for the N most frequent values for each field in the
data. The order and number of values can be tweaked with --reverse, --limit
and --threshold respectively.

Since this computes an exact frequency table, memory proportional to the
cardinality of each column is required.

To compute custom aggregations per group beyond counting, please be sure to
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
    -R, --reverse          Sort the frequency tables in ascending order by
                           count. The default is descending order.
    -N, --no-extra         Don't include empty cells & remaining counts.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 1-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

#[derive(Clone, Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_limit: usize,
    flag_threshold: Option<u64>,
    flag_reverse: bool,
    flag_no_extra: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
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
        let mut items = counter.into_iter().collect::<Vec<_>>();

        if args.flag_reverse {
            items.sort_by(|a, b| a.1.cmp(&b.1));
        } else {
            items.sort_by(|a, b| a.1.cmp(&b.1).reverse());
        }

        let mut remaining: u64 = 0;

        for (i, (value, count)) in items.into_iter().enumerate() {
            if args.flag_limit != 0 && i >= args.flag_limit {
                if args.flag_no_extra {
                    break;
                }

                remaining += count;
                continue;
            }

            if let Some(threshold) = args.flag_threshold {
                if count < threshold {
                    break;
                }
            }

            record.clear();
            record.push_field(&name);
            record.push_field(&value);
            record.push_field(count.to_string().as_bytes());
            wtr.write_byte_record(&record)?;
        }

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
