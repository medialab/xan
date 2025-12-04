use lingua::LanguageDetectorBuilder;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

Usage:
    xan lang [options] <column> [<input>]
    xan lang --help

lang options:

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
    arg_column: SelectedColumns,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    if !rconf.no_headers {
        let mut new_headers = headers.clone();
        new_headers.push_field(b"language");
        wtr.write_byte_record(&new_headers)?;
    }

    let column_index = rconf.single_selection(&headers)?;

    let detector = LanguageDetectorBuilder::from_all_languages().build();

    for result in rdr.byte_records() {
        let mut record = result?;

        let text = std::str::from_utf8(&record[column_index]).unwrap_or("");
        record.push_field(
            detector
                .detect_language_of(text)
                .map(|l| l.to_string())
                .unwrap_or(String::from(""))
                .as_bytes(),
        );

        wtr.write_byte_record(&record)?;
    }

    Ok(wtr.flush()?)
}
