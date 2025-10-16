use bgzip::header;
use bstr::ByteSlice;
use csv::ByteRecord;

use crate::cmd::split;
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

Usage:
    xan separate [options] <columns> <separator> [<input>]
    xan separate --help

separate options:
    -r, --regex  Split cells using a regex pattern instead of the <separator>
                 substring.

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
    arg_separator: String,
    arg_input: Option<String>,
    flag_regex: bool,
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

    let mut records: Vec<ByteRecord> = Vec::new();
    let mut max_splits = 0;

    if !args.flag_regex {
        for result in rdr.byte_records() {
            let record = result?;

            let numsplits = record.as_slice().find_iter(b" ").count() + 1;
            if numsplits > max_splits {
                max_splits = numsplits;
            }

            records.push(record);
        }

        
        let mut new_headers = ByteRecord::from(headers.clone());
        for i in 1..=max_splits {
            let header_name = format!("untitled{}", i);
            new_headers.push_field(header_name.as_bytes());
        }
        wtr.write_byte_record(&new_headers)?;

        for record in records {
            let mut output_record = record.clone();
            let split_record: Vec<Vec<u8>> = record
                .as_slice()
                .split_str(&args.arg_separator)
                .map(|s| s.to_vec())
                .collect();
            for cell in split_record.iter() {
                output_record.push_field(&cell);
            }
            while output_record.len() <= max_splits {
                output_record.push_field(b"");
            }
            wtr.write_byte_record(&output_record)?;
        }
    }

    Ok(wtr.flush()?)
}
