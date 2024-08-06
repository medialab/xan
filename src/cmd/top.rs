use std::cmp::Reverse;
use std::num::NonZeroUsize;

use ordered_float::NotNan;

use crate::collections::{FixedReverseHeapMap, SortedInsertHashmap};
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

type GroupKey = Vec<Vec<u8>>;

// TODO: add rank column?
// TODO: add way to sort group?

static USAGE: &str = "
Find top k CSV rows according to some column values.

Runs in O(N * log k) time, consuming only O(k) memory.

Usage:
    xan top <column> [options] [<input>]
    xan top --help

dedup options:
    -l, --limit <n>       Number of top items to return. Cannot be < 1.
                          [default: 10]
    -R, --reverse         Reverse order.
    -g, --groupby <cols>  Return top n values per group, represented
                          by the values in given columns.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
";

#[derive(PartialEq, PartialOrd, Ord, Eq)]
struct Forward<T>(T);

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_column: SelectColumns,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_limit: NonZeroUsize,
    flag_reverse: bool,
    flag_groupby: Option<SelectColumns>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?;
    let score_col = rconf.single_selection(headers)?;

    let groupby_sel_opt = args
        .flag_groupby
        .map(|cols| Config::new(&None).select(cols).selection(headers))
        .transpose()?;

    let mut wtr = Config::new(&args.flag_output).writer()?;

    rconf.write_headers(&mut rdr, &mut wtr)?;

    macro_rules! run {
        ($type:ident) => {{
            let mut record = csv::ByteRecord::new();
            let mut heap =
                FixedReverseHeapMap::<($type<NotNan<f64>>, usize), csv::ByteRecord>::with_capacity(
                    usize::from(args.flag_limit),
                );

            let mut i: usize = 0;

            while rdr.read_byte_record(&mut record)? {
                if let Ok(score) = std::str::from_utf8(&record[score_col])
                    .unwrap_or("")
                    .parse::<NotNan<f64>>()
                {
                    heap.push_with(($type(score), i), || record.clone());
                    i += 1;
                }
            }

            for (_, record) in heap.into_sorted_vec() {
                wtr.write_byte_record(&record)?;
            }
        }};
    }

    macro_rules! run_groupby {
        ($type:ident, $sel:ident) => {{
            let mut record = csv::ByteRecord::new();
            let mut groups: SortedInsertHashmap<
                GroupKey,
                FixedReverseHeapMap<($type<NotNan<f64>>, usize), csv::ByteRecord>,
            > = SortedInsertHashmap::new();

            let mut i: usize = 0;

            while rdr.read_byte_record(&mut record)? {
                if let Ok(score) = std::str::from_utf8(&record[score_col])
                    .unwrap_or("")
                    .parse::<NotNan<f64>>()
                {
                    let group = $sel
                        .select(&record)
                        .map(|cell| cell.to_vec())
                        .collect::<Vec<_>>();

                    groups.insert_with_or_else(
                        group,
                        || {
                            let mut heap =
                                FixedReverseHeapMap::with_capacity(usize::from(args.flag_limit));
                            heap.push_with(($type(score), i), || record.clone());
                            heap
                        },
                        |mut heap| {
                            heap.push_with(($type(score), i), || record.clone());
                        },
                    );
                }

                i += 1;
            }

            for heap in groups.into_values() {
                for (_, record) in heap.into_sorted_vec() {
                    wtr.write_byte_record(&record)?;
                }
            }
        }};
    }

    match (args.flag_reverse, groupby_sel_opt) {
        (true, None) => run!(Reverse),
        (false, None) => run!(Forward),
        (true, Some(sel)) => run_groupby!(Reverse, sel),
        (false, Some(sel)) => run_groupby!(Forward, sel),
    }

    Ok(wtr.flush()?)
}
