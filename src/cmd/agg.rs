use csv;

use config::{Config, Delimiter};
use util;
use CliResult;

use xan::AggregationProgram;

static USAGE: &str = "
TODO...

Usage:
    xsv agg [options] <expression> [<input>]

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
        .no_headers(args.flag_no_headers);

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;
    let headers = rdr.byte_headers()?;

    let program = AggregationProgram::parse(&args.arg_expression, &headers)?;

    dbg!(&program);

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        // println!("{:?}", &record);
    }

    Ok(wtr.flush()?)
}
