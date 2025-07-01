use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Unpivot a CSV file by allowing multiple columns to be stacked into fewer columns.

For instance, given the following file:

dept,jan,feb,mar
electronics,1,2,3
clothes,10,20,30
cars,100,200,300

The following command:

    $ xan pivot jan: month sales file.csv

Will produce the following result:

dept,month,sales
electronics,jan,1
electronics,feb,2
electronics,mar,3
clothes,jan,10
clothes,feb,20
clothes,mar,30
cars,jan,100
cars,feb,200
cars,mar,300

Usage:
    xan unpivot [options] <columns> <name> <value> [<input>]
    xan unpivot --help

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_columns: SelectColumns,
    arg_name: String,
    arg_value: String,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .delimiter(args.flag_delimiter)
        .select(args.arg_columns);

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();
    let sel = rconf.selection(&headers)?;
    let inverse_sel = sel.inverse(headers.len());

    let mut wtr = Config::new(&args.flag_output).writer()?;

    if !rconf.no_headers {
        let mut output_headers = csv::ByteRecord::new();

        for h in inverse_sel.select(&headers) {
            output_headers.push_field(h);
        }

        output_headers.push_field(args.arg_name.as_bytes());
        output_headers.push_field(args.arg_value.as_bytes());

        wtr.write_byte_record(&output_headers)?;
    }

    let mut record = csv::ByteRecord::new();
    let mut output_record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        output_record.clear();

        for cell in inverse_sel.select(&record) {
            output_record.push_field(cell);
        }

        for (name, value) in sel.select(&headers).zip(sel.select(&record)) {
            output_record.truncate(inverse_sel.len());
            output_record.push_field(name);
            output_record.push_field(value);

            wtr.write_byte_record(&output_record)?;
        }
    }

    Ok(wtr.flush()?)
}
