use crate::config::{Config, Delimiter};
use crate::moonblade::WindowAggregationProgram;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
TODO...

Usage:
    xan window [options] <expression> [<input>]
    xan window --help

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
    arg_expression: String,
    arg_input: Option<String>,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut reader = conf.reader()?;
    let mut writer = Config::new(&args.flag_output).writer()?;

    let headers = reader.byte_headers()?.clone();
    let mut program = WindowAggregationProgram::parse(&args.arg_expression, &headers)?;

    if !args.flag_no_headers {
        writer.write_record(headers.iter().chain(program.headers()))?;
    }

    let mut record = csv::ByteRecord::new();
    let mut index: usize = 0;

    while reader.read_byte_record(&mut record)? {
        if let Some(output_record) = program.run_with_record(index, &record)? {
            writer.write_byte_record(&output_record)?;
        }

        index += 1;
    }

    for output_record in program.flush()? {
        writer.write_byte_record(&output_record)?;
    }

    Ok(writer.flush()?)
}
