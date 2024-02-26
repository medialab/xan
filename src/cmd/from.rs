use std::num::NonZeroUsize;
use std::{
    fs,
    io::{self, BufRead, BufReader, Cursor, Read},
    path::Path,
};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use csv;
use serde::de::{Deserialize, Deserializer, Error};
use serde_json::Value;

use config::Config;
use json::for_each_json_value_as_csv_record;
use util;
use CliError;
use CliResult;

// TODO: support json array value as paths or serialize them with a separator

#[derive(Debug, Clone, Copy)]
enum SupportedFormat {
    Xls,
    NdJSON,
    JSONArray,
}

impl SupportedFormat {
    fn parse(string: &str) -> Option<Self> {
        Some(match string {
            "xls" | "xlsx" | "xlsb" | "ods" => Self::Xls,
            "jsonl" | "ndjson" => Self::NdJSON,
            "json" | "json-array" => Self::JSONArray,
            _ => return None,
        })
    }

    fn infer_from_extension(path: &str) -> Option<Self> {
        Self::parse(
            Path::new(path)
                .extension()
                .map(|e| e.to_str().unwrap())
                .unwrap_or(""),
        )
    }
}

impl<'de> Deserialize<'de> for SupportedFormat {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<SupportedFormat, D::Error> {
        let raw = String::deserialize(d)?;

        SupportedFormat::parse(&raw)
            .ok_or_else(|| D::Error::custom(format!("unknown format \"{}\"", &raw)))
    }
}

static USAGE: &str = "
Convert a variety of data formats to CSV.

Usage:
    xan from [options] [<input>]
    xan from --help

Supported formats:
    ods   - OpenOffice spreadsheet
    xls   - Excel spreasheet
    xlsb  - Excel spreasheet
    xlsx  - Excel spreasheet

    json       - JSON array
    json-array - JSON array
    ndjson     - Newline-delimited JSON
    jsonl      - Newline-delimited JSON

from options:
    -f, --format <format>  Format to convert from. Will be inferred from file
                           extension if not given. Must be specified when reading
                           from stdin, since we don't have a file extension to
                           work with.

Excel/OpenOffice-related options:
    -s, --sheet <name>     Name of the sheet to convert. [default: Sheet1]

JSON options:
    --sample-size <n>      Number of records to sample before emitting headers.
                           [default: 64]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_sheet: String,
    flag_format: Option<SupportedFormat>,
    flag_output: Option<String>,
    flag_sample_size: NonZeroUsize,
}

impl Args {
    fn writer(&self) -> io::Result<csv::Writer<Box<dyn io::Write>>> {
        Config::new(&self.flag_output).writer()
    }

    fn convert_xls(&self) -> CliResult<()> {
        let reader = Cursor::new(match self.arg_input.as_ref() {
            None => {
                let mut contents = Vec::<u8>::new();
                io::stdin().read_to_end(&mut contents)?;
                contents
            }
            Some(path) => {
                let mut contents = Vec::<u8>::new();
                fs::File::open(path)?.read_to_end(&mut contents)?;
                contents
            }
        });

        let mut wtr = self.writer()?;

        let mut workbook = open_workbook_auto_from_rs(reader)?;
        let mut record = csv::StringRecord::new();

        let range = workbook.worksheet_range(&self.flag_sheet);

        match range {
            Err(_) => {
                let sheets = workbook.sheet_names().join(", ");

                return Err(CliError::Other(format!(
                    "could not find the \"{}\" sheet\nshould be one of: {}",
                    &self.flag_sheet, sheets
                )));
            }
            Ok(range) => {
                for row in range.rows() {
                    record.clear();

                    for cell in row {
                        match cell {
                            Data::String(value) => record.push_field(value),
                            Data::DateTimeIso(value) => record.push_field(value),
                            Data::DurationIso(value) => record.push_field(value),
                            Data::Bool(value) => {
                                record.push_field(if *value { "true" } else { "false" })
                            }
                            Data::Int(value) => record.push_field(&value.to_string()),
                            Data::Float(value) => record.push_field(&value.to_string()),
                            Data::DateTime(value) => record.push_field(&value.to_string()),
                            Data::Error(err) => record.push_field(&err.to_string()),
                            Data::Empty => record.push_field(""),
                        }
                    }

                    wtr.write_record(&record)?;
                }
            }
        }

        Ok(wtr.flush()?)
    }

    fn convert_ndjson(&self) -> CliResult<()> {
        let mut wtr = self.writer()?;

        let rdr: Box<dyn BufRead> = match self.arg_input.as_ref() {
            None => Box::new(BufReader::new(io::stdin())),
            Some(p) => Box::new(BufReader::new(fs::File::open(p)?)),
        };

        for_each_json_value_as_csv_record(
            rdr.lines().map(|line| -> Result<Value, CliError> {
                serde_json::from_str(&line?).map_err(|err| CliError::Other(err.to_string()))
            }),
            self.flag_sample_size,
            |record| -> CliResult<()> {
                wtr.write_record(record)?;
                Ok(())
            },
        )?;

        Ok(wtr.flush()?)
    }

    fn convert_json_array(&self) -> CliResult<()> {
        let mut rdr: Box<dyn Read> = match self.arg_input.as_ref() {
            None => Box::new(io::stdin()),
            Some(p) => Box::new(fs::File::open(p)?),
        };

        let mut contents = String::new();
        rdr.read_to_string(&mut contents)?;

        let value =
            serde_json::from_str(&contents).map_err(|err| CliError::Other(err.to_string()))?;

        if let Value::Array(array) = value {
            let mut wtr = self.writer()?;

            for_each_json_value_as_csv_record(
                array.into_iter().map(|v| Ok(v)),
                self.flag_sample_size,
                |record| -> CliResult<()> {
                    wtr.write_record(record)?;
                    Ok(())
                },
            )?;

            Ok(wtr.flush()?)
        } else {
            Err(CliError::Other(
                "target JSON does not contain an array".to_string(),
            ))
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let target_format = {
        if let Some(format) = args.flag_format {
            format
        } else {
            if let Some(p) = args.arg_input.as_ref() {
                match SupportedFormat::infer_from_extension(p) {
                    Some(format) => format,
                    None => {
                        return Err(CliError::Other(
                            "could not infer format from extension.".to_string(),
                        ));
                    }
                }
            } else {
                return Err(CliError::Other(
                    "cannot infer format from stdin. Please provide the -f/--format flag."
                        .to_string(),
                ));
            }
        }
    };

    match target_format {
        SupportedFormat::Xls => args.convert_xls(),
        SupportedFormat::NdJSON => args.convert_ndjson(),
        SupportedFormat::JSONArray => args.convert_json_array(),
    }
}
