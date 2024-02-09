use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};
use csv;

use config::Config;
use util;
use CliError;
use CliResult;

static USAGE: &str = "
Convert an Excel/OpenOffice spreadsheet (.xls, .xlsx, .ods etc.) to CSV.

Usage:
    xan xls [options] <input>
    xan xls --help

xls options:
    -s, --sheet <name>     Name of the sheet to convert. [default: Sheet1]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
";

#[derive(Deserialize)]
struct Args {
    arg_input: String,
    flag_sheet: String,
    flag_output: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut record = csv::ByteRecord::new();

    let mut workbook =
        open_workbook_auto(Path::new(&args.arg_input)).map_err(|_| "could not open spreadsheet")?;

    let range = workbook.worksheet_range(&args.flag_sheet);

    match range {
        Err(_) => {
            let sheets = workbook.sheet_names().join(", ");

            return Err(CliError::Other(format!(
                "could not find the \"{}\" sheet\nshould be one of: {}",
                &args.flag_sheet, sheets
            )));
        }
        Ok(range) => {
            for row in range.rows() {
                record.clear();

                for cell in row {
                    match cell {
                        Data::String(value) => record.push_field(value.as_bytes()),
                        Data::DateTimeIso(value) => record.push_field(value.as_bytes()),
                        Data::DurationIso(value) => record.push_field(value.as_bytes()),
                        Data::Bool(value) => {
                            record.push_field(if *value { b"true" } else { b"false" })
                        }
                        Data::Int(value) => record.push_field(value.to_string().as_bytes()),
                        Data::Float(value) => record.push_field(value.to_string().as_bytes()),
                        Data::DateTime(value) => record.push_field(value.to_string().as_bytes()),
                        Data::Error(err) => record.push_field(err.to_string().as_bytes()),
                        Data::Empty => record.push_field(b""),
                    }
                }

                wtr.write_byte_record(&record)?;
            }
        }
    }

    Ok(wtr.flush()?)
}
