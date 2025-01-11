use crate::collections::UnionFindMap;
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Apply the union-find algorithm on a CSV file representing a graph's
edge list (one column for source nodes, one column for target nodes) in
order to return a CSV of nodes with a component label.

The command can also return only the nodes belonging to the largest connected
component using the -L/--largest flag or the sizes of all the connected
components of the graph using the -S/--sizes flag.

Usage:
    xan union-find <source> <target> [options] [<input>]
    xan union-find --help

union-find options:
    -L, --largest  Only return nodes belonging to the largest component.
                   The output CSV file will only contain a 'node' column in
                   this case.
    -S, --sizes    Return a single CSV column containing the sizes of the graph's
                   various connected components.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_source: SelectColumns,
    arg_target: SelectColumns,
    flag_largest: bool,
    flag_sizes: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = conf.reader()?;

    let headers = rdr.byte_headers()?;

    let source_index = args
        .arg_source
        .single_selection(headers, !args.flag_no_headers)?;

    let target_index = args
        .arg_target
        .single_selection(headers, !args.flag_no_headers)?;

    let mut wtr = Config::new(&args.flag_output).writer()?;
    let mut record = csv::ByteRecord::new();

    let mut union_find = UnionFindMap::<Vec<u8>>::new();

    while rdr.read_byte_record(&mut record)? {
        let source = record[source_index].to_vec();
        let target = record[target_index].to_vec();

        union_find.union(source, target);
    }

    record.clear();

    if args.flag_sizes {
        record.push_field(b"size");
    } else {
        record.push_field(b"node");

        if !args.flag_largest {
            record.push_field(b"component");
        }
    }

    wtr.write_byte_record(&record)?;

    if args.flag_largest {
        if union_find.is_empty() {
            return Ok(wtr.flush()?);
        }

        for node in union_find.largest_component() {
            record.clear();
            record.push_field(&node);

            wtr.write_byte_record(&record)?;
        }
    } else if args.flag_sizes {
        for size in union_find.sizes() {
            record.clear();
            record.push_field(size.to_string().as_bytes());

            wtr.write_byte_record(&record)?;
        }
    } else {
        for (node, label) in union_find.nodes() {
            record.clear();
            record.push_field(&node);
            record.push_field(label.to_string().as_bytes());

            wtr.write_byte_record(&record)?;
        }
    }

    Ok(wtr.flush()?)
}
