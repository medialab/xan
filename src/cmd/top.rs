use std::cmp::Reverse;
use std::iter::once;
use std::num::NonZeroUsize;

use ordered_float::NotNan;
use simd_csv::ByteRecord;

use crate::collections::{ClusteredInsertHashmap, Forward, TopKHeapMap, TopKHeapMapWithTies};
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
enum Value {
    Float(NotNan<f64>),
    String(Vec<u8>),
}

impl Value {
    fn new_float(cell: &[u8]) -> Option<Self> {
        fast_float::parse::<f64, &[u8]>(cell)
            .ok()
            .and_then(|f| NotNan::new(f).ok())
            .map(Self::Float)
    }

    fn new_string(cell: &[u8]) -> Option<Self> {
        Some(Self::String(cell.to_vec()))
    }
}

static USAGE: &str = "
Find top k values in selected column and return the associated CSV rows.

Runs in O(n * log k) time, n being the number of rows in target CSV file, and
consuming only O(k) memory, which is of course better than piping `xan sort`
into `xan head`.

Note that rows having empty values or values that cannot be parsed as numbers
in selected columns will be ignored along the way.

This command can also return the first k values or last k values in lexicographic
order using the -L/--lexicographic flag (note that the logic of the command is
tailored for numerical values and is therefore the reverse of `xan sort` in this
regard).

Examples:

Top 10 values in \"score\" column:

    $ xan top score file.csv

Top 50 values:

    $ xan top -l 50 score file.csv

Smallest 10 values:

    $ xan top -R score file.csv

Top 10 values with potential ties:

    $ xan top -T score file.csv

Top 10 values per distinct value of the \"category\" column:

    $ xan top -g category score file.csv

The same with a preprended \"rank\" column:

    $ xan top -g category -r rank score file.csv

Last 10 names in lexicographic order:

    $ xan top -L name file.csv

First 10 names in lexicographic order:

    $ xan top -LR name file.csv

Usage:
    xan top <column> [options] [<input>]
    xan top --help

top options:
    -l, --limit <n>       Number of top items to return. Cannot be < 1.
                          [default: 10]
    -R, --reverse         Reverse order.
    -L, --lexicographic   Rank values lexicographically instead of considering
                          them as numbers.
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

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_column: SelectedColumns,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_limit: NonZeroUsize,
    flag_reverse: bool,
    flag_groupby: Option<SelectedColumns>,
    flag_rank: Option<String>,
    flag_ties: bool,
    flag_lexicographic: bool,
}

impl Args {
    fn new_value(&self, cell: &[u8]) -> Option<Value> {
        if self.flag_lexicographic {
            Value::new_string(cell)
        } else {
            Value::new_float(cell)
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column.clone());

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?;
    let score_col = rconf.single_selection(headers)?;

    let groupby_sel_opt = args
        .flag_groupby
        .as_ref()
        .map(|cols| cols.selection(headers, !rconf.no_headers))
        .transpose()?;

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconf.no_headers {
        if let Some(name) = &args.flag_rank {
            wtr.write_record(once(name.as_bytes()).chain(headers.iter()))?;
        } else {
            wtr.write_byte_record(headers)?;
        }
    }

    macro_rules! run {
        ($heap:ident, $type:ident) => {{
            let mut record = ByteRecord::new();
            let mut heap =
                $heap::<$type<Value>, ByteRecord>::with_capacity(usize::from(args.flag_limit));

            while rdr.read_byte_record(&mut record)? {
                if let Some(score) = args.new_value(&record[score_col]) {
                    heap.push_with($type(score), || record.clone());
                }
            }

            for (i, (_, record)) in heap.into_sorted_vec().into_iter().enumerate() {
                if args.flag_rank.is_some() {
                    wtr.write_record(once((i + 1).to_string().as_bytes()).chain(record.iter()))?;
                } else {
                    wtr.write_byte_record(&record)?;
                }
            }
        }};
    }

    macro_rules! run_groupby {
        ($heap:ident, $type:ident, $sel:ident) => {{
            let mut record = ByteRecord::new();
            let mut groups: ClusteredInsertHashmap<ByteRecord, $heap<$type<Value>, ByteRecord>> =
                ClusteredInsertHashmap::new();

            while rdr.read_byte_record(&mut record)? {
                if let Some(score) = args.new_value(&record[score_col]) {
                    let group = $sel.select(&record).collect();

                    let heap = groups
                        .insert_with(group, || $heap::with_capacity(usize::from(args.flag_limit)));

                    heap.push_with($type(score), || record.clone());
                }
            }

            for heap in groups.into_values() {
                for (i, (_, record)) in heap.into_sorted_vec().into_iter().enumerate() {
                    if args.flag_rank.is_some() {
                        wtr.write_record(
                            once((i + 1).to_string().as_bytes()).chain(record.iter()),
                        )?;
                    } else {
                        wtr.write_byte_record(&record)?;
                    }
                }
            }
        }};
    }

    match (args.flag_reverse, args.flag_ties, groupby_sel_opt) {
        (true, false, None) => run!(TopKHeapMap, Reverse),
        (false, false, None) => run!(TopKHeapMap, Forward),
        (true, false, Some(sel)) => run_groupby!(TopKHeapMap, Reverse, sel),
        (false, false, Some(sel)) => run_groupby!(TopKHeapMap, Forward, sel),
        (true, true, None) => run!(TopKHeapMapWithTies, Reverse),
        (false, true, None) => run!(TopKHeapMapWithTies, Forward),
        (true, true, Some(sel)) => run_groupby!(TopKHeapMapWithTies, Reverse, sel),
        (false, true, Some(sel)) => run_groupby!(TopKHeapMapWithTies, Forward, sel),
    };

    Ok(wtr.flush()?)
}
