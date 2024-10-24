use std::cmp::Reverse;
use std::num::NonZeroUsize;

use ordered_float::NotNan;

use crate::collections::{
    ClusteredInsertHashmap, FixedReverseHeapMap, FixedReverseHeapMapWithTies,
};
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

type GroupKey = Vec<Vec<u8>>;

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
    -r, --rank <col>      Name of a rank column to prepend.
    -T, --ties            Keep all rows tied for last. Will therefore
                          consume O(k + t) memory, t being the number of ties.

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
    flag_rank: Option<String>,
    flag_ties: bool,
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

    if !args.flag_no_headers {
        if let Some(name) = &args.flag_rank {
            wtr.write_byte_record(&headers.prepend(name.as_bytes()))?;
        } else {
            wtr.write_byte_record(headers)?;
        }
    }

    macro_rules! run {
        ($heap:ident, $type:ident) => {{
            let mut record = csv::ByteRecord::new();
            let mut heap = $heap::<$type<NotNan<f64>>, csv::ByteRecord>::with_capacity(
                usize::from(args.flag_limit),
            );

            while rdr.read_byte_record(&mut record)? {
                if let Ok(score) = std::str::from_utf8(&record[score_col])
                    .unwrap_or("")
                    .parse::<NotNan<f64>>()
                {
                    heap.push_with($type(score), || record.clone());
                }
            }

            for (i, (_, record)) in heap.into_sorted_vec().into_iter().enumerate() {
                if args.flag_rank.is_some() {
                    wtr.write_byte_record(&record.prepend((i + 1).to_string().as_bytes()))?;
                } else {
                    wtr.write_byte_record(&record)?;
                }
            }
        }};
    }

    macro_rules! run_groupby {
        ($heap:ident, $type:ident, $sel:ident) => {{
            let mut record = csv::ByteRecord::new();
            let mut groups: ClusteredInsertHashmap<
                GroupKey,
                $heap<$type<NotNan<f64>>, csv::ByteRecord>,
            > = ClusteredInsertHashmap::new();

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
                            let mut heap = $heap::with_capacity(usize::from(args.flag_limit));
                            heap.push_with($type(score), || record.clone());
                            heap
                        },
                        |heap| {
                            heap.push_with($type(score), || record.clone());
                        },
                    );
                }
            }

            for heap in groups.into_values() {
                for (i, (_, record)) in heap.into_sorted_vec().into_iter().enumerate() {
                    if args.flag_rank.is_some() {
                        wtr.write_byte_record(&record.prepend((i + 1).to_string().as_bytes()))?;
                    } else {
                        wtr.write_byte_record(&record)?;
                    }
                }
            }
        }};
    }

    match (args.flag_reverse, args.flag_ties, groupby_sel_opt) {
        (true, false, None) => run!(FixedReverseHeapMap, Reverse),
        (false, false, None) => run!(FixedReverseHeapMap, Forward),
        (true, false, Some(sel)) => run_groupby!(FixedReverseHeapMap, Reverse, sel),
        (false, false, Some(sel)) => run_groupby!(FixedReverseHeapMap, Forward, sel),
        (true, true, None) => run!(FixedReverseHeapMapWithTies, Reverse),
        (false, true, None) => run!(FixedReverseHeapMapWithTies, Forward),
        (true, true, Some(sel)) => run_groupby!(FixedReverseHeapMapWithTies, Reverse, sel),
        (false, true, Some(sel)) => run_groupby!(FixedReverseHeapMapWithTies, Forward, sel),
    };

    Ok(wtr.flush()?)
}
