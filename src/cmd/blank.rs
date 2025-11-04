use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

// TODO: some --pivot option, that blanks hierarchically

static USAGE: &str = "
Blank down selected columns of a CSV file. That is to say, this
command will redact any consecutive identical cells as per column selection.

This can be useful as a presentation trick or a compression scheme.

The \"blank\" term comes from OpenRefine and does the same thing.

Usage:
    xan blank [options] [<input>]
    xan blank --help

blank options:
    -s, --select <cols>    Selection of columns to blank down.
    -r, --redact <value>   Redact the blanked down values using the provided
                           replacement string. Will default to an empty string.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectedColumns,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_redact: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let redacted_string = args.flag_redact.unwrap_or("".to_string());

    let mut rdr = rconf.simd_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let headers = rdr.byte_headers()?;

    let sel = rconf.selection(headers)?;
    let mask = sel.mask(headers.len());

    if !rconf.no_headers {
        wtr.write_byte_record(headers)?;
    }

    let mut record = ByteRecord::new();
    let mut current: Option<ByteRecord> = None;

    while rdr.read_byte_record(&mut record)? {
        let key = sel.select(&record).collect::<ByteRecord>();

        match current.as_ref() {
            Some(current_key) if current_key == &key => {
                wtr.write_record(mask.iter().copied().zip(record.iter()).map(
                    |(should_redact, cell)| {
                        if should_redact {
                            redacted_string.as_bytes()
                        } else {
                            cell
                        }
                    },
                ))?;
            }
            _ => {
                current = Some(key);
                wtr.write_byte_record(&record)?;
            }
        }
    }

    Ok(wtr.flush()?)
}
