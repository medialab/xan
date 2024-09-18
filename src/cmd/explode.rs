use bstr::ByteSlice;
use csv;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliError;
use crate::CliResult;

static USAGE: &str = "
Explode CSV rows into multiple ones by splitting column values by using the
provided separator.

This is the reverse of the 'implode' command.

For instance the following CSV:

name,colors
John,blue|yellow
Mary,red

Can be exploded on the \"colors\" column using the \"|\" <separator> to produce:

name,colors
John,blue
John,yellow
Mary,red

Note finally that the file can be exploded on multiple well-aligned columns.

Usage:
    xan explode [options] <columns> <separator> [<input>]
    xan explode --help

explode options:
    -r, --rename <name>    New names for the exploded columns. Must be written
                           in CSV format if exploding multiple columns.
                           See 'xsv rename' help for more details.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_columns: SelectColumns,
    arg_separator: String,
    arg_input: Option<String>,
    flag_rename: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    if sel.is_empty() {
        return Err(CliError::Other(
            "expecting a non-empty column selection".to_string(),
        ));
    }

    // NOTE: the mask deduplicates
    let sel_mask = sel.indexed_mask(headers.len());

    if let Some(new_names) = args.flag_rename {
        let new_names = util::str_to_csv_byte_record(&new_names);

        if new_names.len() != sel.len() {
            return fail!(format!(
                "Renamed columns alignement error. Expected {} names and got {}.",
                sel.len(),
                new_names.len(),
            ));
        }

        headers = headers
            .iter()
            .zip(sel_mask.iter())
            .map(|(h, rh)| if let Some(i) = rh { &new_names[*i] } else { h })
            .collect();
    }

    if !rconfig.no_headers {
        wtr.write_byte_record(&headers)?;
    }

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        let splits: Vec<Vec<&[u8]>> = sel
            .select(&record)
            .map(|cell| cell.split_str(&args.arg_separator).collect())
            .collect();

        if splits.iter().skip(1).any(|s| s.len() != splits[0].len()) {
            return Err(CliError::Other(
                "inconsistent exploded length accross columns.".to_string(),
            ));
        }

        if splits[0].is_empty() {
            wtr.write_byte_record(&record)?;
            continue;
        }

        for i in 0..splits[0].len() {
            let output_record: csv::ByteRecord = record
                .iter()
                .zip(sel_mask.iter())
                .map(|(cell, mask)| {
                    if let Some(j) = mask {
                        splits[*j][i]
                    } else {
                        cell
                    }
                })
                .collect();

            wtr.write_byte_record(&output_record)?;
        }
    }

    Ok(wtr.flush()?)
}
