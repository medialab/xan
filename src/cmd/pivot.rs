use std::collections::{btree_map::Entry, BTreeMap};

use crate::config::{Config, Delimiter};
use crate::moonblade::AggregationProgram;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

// TODO: IN, groupby, multiselect
// TODO: create a PivotAggregationProgram, mapping group key (that can be refined through --groupby)
// to BTreeMaps of aggregators.

static USAGE: &str = r#"
TODO...

Usage:
    xan pivot [-P...] [options] <column> <expr> [<input>]
    xan pivot --help

pivot options:
    -P  Use at least three times for greater effect!

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize, Debug)]
struct Args {
    arg_input: Option<String>,
    arg_column: SelectColumns,
    arg_expr: String,
    #[serde(rename = "flag_P")]
    flag_p: usize,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_p >= 3 {
        println!("{}", include_str!("../moonblade/doc/pivot.txt"));
        return Ok(());
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter)
        .select(args.arg_column);

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();
    let pivot_col_index = rconf.single_selection(&headers)?;
    let program = AggregationProgram::parse(&args.arg_expr, &headers)?;

    if !program.has_single_expr() {
        Err("expected a single aggregation clause!")?;
    }

    let column_indices_used_in_aggregation = program.used_column_indices();

    if column_indices_used_in_aggregation.contains(&pivot_col_index) {
        Err("aggregation cannot work on the pivot column!")?;
    }

    let mut pivot_map: BTreeMap<Vec<u8>, Vec<csv::ByteRecord>> = BTreeMap::new();

    let mut wtr = Config::new(&args.flag_output).writer()?;

    for result in rdr.byte_records() {
        let record = result?;

        let key = record[pivot_col_index].to_vec();

        match pivot_map.entry(key) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().push(record);
            }
            Entry::Vacant(entry) => {
                entry.insert(vec![record]);
            }
        };
    }

    if !rconf.no_headers {
        let mut output_headers = headers
            .iter()
            .enumerate()
            .filter_map(|(i, h)| {
                if i != pivot_col_index && !column_indices_used_in_aggregation.contains(&i) {
                    Some(h)
                } else {
                    None
                }
            })
            .collect::<csv::ByteRecord>();

        for key in pivot_map.keys() {
            output_headers.push_field(key);
        }

        wtr.write_byte_record(&output_headers)?;
    }

    Ok(wtr.flush()?)
}
