use crate::config::{Config, Delimiter};
use crate::moonblade::WindowAggregationProgram;
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
TODO...

Usage:
    xan window [options] <expression> [<input>]
    xan window --help

window options:
    -g, --groupby <cols>  If given, resets the computed aggregations each
                          time the given selection yields a new identity.

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
    flag_groupby: Option<SelectColumns>,
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

    let groupby_sel_opt = args
        .flag_groupby
        .map(|s| s.selection(&headers, !args.flag_no_headers))
        .transpose()?;

    if !args.flag_no_headers {
        writer.write_record(headers.iter().chain(program.headers()))?;
    }

    let mut record = csv::ByteRecord::new();
    let mut index: usize = 0;
    let mut group_opt: Option<Vec<Vec<u8>>> = None;

    while reader.read_byte_record(&mut record)? {
        if let Some(sel) = &groupby_sel_opt {
            match &mut group_opt {
                None => {
                    group_opt = Some(sel.collect(&record));
                }
                Some(group) => {
                    let new_group = sel.collect(&record);

                    if group != &new_group {
                        for output_record in program.flush_and_clear()? {
                            writer.write_byte_record(&output_record)?;
                        }

                        *group = new_group;
                    }
                }
            };
        }

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
