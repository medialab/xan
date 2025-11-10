use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
TODO...

Usage:
    xan bisect [options] <column> <value> [<input>]
    xan bisect --help

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
    arg_column: SelectedColumns,
    arg_value: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let target_value: u64 = args.arg_value.parse().unwrap();

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column)
        .delimiter(args.flag_delimiter);

    let mut seek_rdr = rconf.simd_seeker()?.unwrap();

    let column_index = rconf.single_selection(rconf.reader()?.byte_headers()?)?;

    let mut median_byte = seek_rdr.file_len() / 2;
    let mut start_byte = seek_rdr.first_record_position();
    let mut end_byte = seek_rdr.file_len();

    let mut previous_median: Option<u64> = None;

    let mut value: u64;

    let mut record: ByteRecord;
    let mut record_pos: u64 = start_byte;

    while start_byte <= end_byte {
        (record_pos, record) = seek_rdr.seek(median_byte)?.unwrap();

        value = std::str::from_utf8(&record[column_index])
            .unwrap()
            .parse::<u64>()
            .unwrap();

        if value == target_value {
            break;
        } else if value < target_value {
            // move start byte up
            start_byte = record_pos;
        } else {
            // move end byte down
            end_byte = record_pos;
        }

        if let Some(prev) = previous_median {
            if prev == median_byte {
                println!("Value {} not found in file", target_value);
                return Ok(());
            }
        }
        previous_median = Some(median_byte);
        median_byte = (start_byte + end_byte) / 2;
    }

    println!(
        "Found value {} at byte position {}",
        target_value, record_pos
    );
    Ok(())
}
