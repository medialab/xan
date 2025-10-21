use csv::StringRecord;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

Usage:
    xan complete [options] <columns> [<input>]
    xan complete --help

complete options:

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
    arg_columns: SelectColumns,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let sel = rconf.selection(&headers)?;
    let mut index: Option<i32> = None;

    let mut record = StringRecord::new();

    wtr.write_record(&headers)?;

    while rdr.read_record(&mut record)? {
        let value = sel
            .select(&record)
            .map(|i| i.parse::<i32>().unwrap())
            .next();

        while index.is_some() && value.unwrap() > index.unwrap() {
            let mut new_record = StringRecord::new();
            for cell in sel.indexed_mask(record.len()) {
                if cell.is_some() {
                    new_record.push_field(&index.unwrap().to_string());
                } else {
                    new_record.push_field("");
                }
            }
            index = Some(index.unwrap() + 1);
            wtr.write_record(&new_record)?;
        }

        index = Some(value.unwrap() + 1);
        wtr.write_record(&record)?;
    }

    Ok(wtr.flush()?)
}
