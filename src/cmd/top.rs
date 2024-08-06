use std::cmp::Reverse;
use std::num::NonZeroUsize;

use ordered_float::NotNan;

use crate::collections::FixedReverseHeapMap;
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Find top k CSV rows according to some column values.

Run in O(n log k) time, consuming only O(k) memory.

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
    let score_col = rconf.single_selection(rdr.byte_headers()?)?;

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

    if args.flag_reverse {
        run!(Reverse);
    } else {
        run!(Forward);
    };

    Ok(wtr.flush()?)
}
