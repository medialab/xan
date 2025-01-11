use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
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
    flag_select: SelectColumns,
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

    let mut rdr = rconf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?;

    let sel = rconf.selection(headers)?;
    let mask = sel.indexed_mask(headers.len());

    rconf.write_headers(&mut rdr, &mut wtr)?;

    let mut record = csv::ByteRecord::new();
    let mut current: Option<Vec<Vec<u8>>> = None;

    while rdr.read_byte_record(&mut record)? {
        let key = sel
            .select(&record)
            .map(|cell| cell.to_vec())
            .collect::<Vec<_>>();

        match current.as_ref() {
            Some(current_key) if current_key == &key => {
                let redacted_record = mask
                    .iter()
                    .zip(record.iter())
                    .map(|(opt, cell)| {
                        if opt.is_some() {
                            redacted_string.as_bytes()
                        } else {
                            cell
                        }
                    })
                    .collect::<csv::ByteRecord>();

                wtr.write_byte_record(&redacted_record)?;
            }
            _ => {
                current = Some(key);
                wtr.write_byte_record(&record)?;
            }
        }
    }

    Ok(wtr.flush()?)
}
