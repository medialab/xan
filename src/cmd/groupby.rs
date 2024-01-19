use csv;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliResult;

use xan::GroupAggregationProgram;

static USAGE: &str = "
TODO...

Usage:
    xsv groupby [options] <column> <expression> [<input>]
    xsv groupby --help

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_column: SelectColumns,
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let column_index = rconf.single_selection(&headers)?;

    let mut program = GroupAggregationProgram::parse(&args.arg_expression, &headers)?;

    let mut record = csv::ByteRecord::new();

    wtr.write_byte_record(&program.headers())?;

    while rdr.read_byte_record(&mut record)? {
        let group = record[column_index].to_vec();
        program.run_with_record(group, &record)?;
    }

    program.finalize(|output_record| wtr.write_byte_record(output_record))?;

    Ok(wtr.flush()?)
}
