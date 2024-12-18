use std::{
    fs,
    io::{self, IsTerminal, Read, Write},
};

use csv::{self, StringRecord};
use rust_xlsxwriter::Workbook;
use serde_json::Value;

use crate::config::Config;
use crate::json::{infer_json_type, JSONTypeInferrenceMode};
use crate::util;
use crate::CliError;
use crate::CliResult;

static USAGE: &str = "
Convert a CSV file to a variety of data formats.

Usage:
    xan to [<format>] [options] [<input>]
    xan to --help

Supported formats:
    json    - JSON array or object
    ndjson  - Newline-delimited JSON
    jsonl   - Newline-delimited JSON
    xlsx    - Excel spreasheet

JSON options:
    --nulls            Convert empty string to a null value.
    --omit          Ignore the empty values.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
";

#[derive(Deserialize)]
struct Args {
    arg_format: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_nulls: bool,
    flag_omit: bool,
}

impl Args {
    fn json_type_inferrence_mode(&self) -> JSONTypeInferrenceMode {
        if self.flag_nulls {
            JSONTypeInferrenceMode::Null
        } else if self.flag_omit {
            JSONTypeInferrenceMode::Omit
        } else {
            JSONTypeInferrenceMode::Empty
        }
    }

    fn make_json(
        &self,
        record: &StringRecord,
        headers: &StringRecord,
        json_object: &mut serde_json::Map<String, Value>,
        mode: JSONTypeInferrenceMode,
    ) {
        for (header, value) in headers.iter().zip(record.iter()) {
            if let Some(json_value) = infer_json_type(value, mode) {
                json_object.insert(header.to_string(), json_value);
            } else {
                json_object.remove(header);
            }
        }
    }

    fn convert_to_json<R: Read, W: Write>(
        &self,
        mut rdr: csv::Reader<R>,
        writer: W,
    ) -> CliResult<()> {
        let headers = rdr.headers()?.clone();
        let mut record = csv::StringRecord::new();
        let mut json_object = serde_json::Map::new();
        let mode = self.json_type_inferrence_mode();

        let mut json_array = Vec::new();

        while rdr.read_record(&mut record)? {
            self.make_json(&record, &headers, &mut json_object, mode);

            json_array.push(Value::Object(json_object.clone()));
        }
        let _ = serde_json::to_writer_pretty(writer, &json_array);

        Ok(())
    }

    fn convert_to_ndjson<R: Read, W: Write>(
        &self,
        mut rdr: csv::Reader<R>,
        mut writer: W,
    ) -> CliResult<()> {
        let headers = rdr.headers()?.clone();
        let mut record = csv::StringRecord::new();
        let mut json_object = serde_json::Map::new();
        let mode = self.json_type_inferrence_mode();

        while rdr.read_record(&mut record)? {
            self.make_json(&record, &headers, &mut json_object, mode);

            writeln!(
                writer,
                "{}",
                serde_json::to_string(&json_object).map_err(|e| CliError::Other(e.to_string()))?
            )?;
        }

        Ok(())
    }

    fn convert_to_xlsx<R: Read>(
        mut rdr: csv::Reader<R>,
        mut writer: Box<dyn Write>,
    ) -> CliResult<()> {
        let mut workbook = Workbook::new();
        let headers = rdr.headers()?.clone();
        let worksheet = workbook.add_worksheet();

        for (col, header) in headers.iter().enumerate() {
            worksheet.write_string(0, col as u16, header)?;
        }

        for (row, value) in rdr.records().enumerate() {
            let record = value?;
            for (col, field) in record.iter().enumerate() {
                worksheet.write_string((row + 1) as u32, col as u16, field)?;
            }
        }

        let mut cursor = io::Cursor::new(Vec::new());
        workbook.save_to_writer(&mut cursor)?;
        let buf = cursor.into_inner();
        writer.write_all(&buf)?;

        writer.flush()?;
        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input);
    let rdr = conf.reader()?;

    let writer: Box<dyn Write> = match &args.flag_output {
        Some(output_path) => Box::new(fs::File::create(output_path)?),
        None => Box::new(io::stdout()),
    };

    match args.arg_format.as_str() {
        "json" => Args::convert_to_json(&args, rdr, writer)?,
        "jsonl" | "ndjson" => Args::convert_to_ndjson(&args, rdr, writer)?,
        "xlsx" => {
            if args.flag_output.is_some() || !io::stdout().is_terminal() {
                Args::convert_to_xlsx(rdr, writer)?;
            } else {
                return fail!(
                    "could not export in xlsx without a path, use -o, --output or pipe the result!"
                );
            }
        }
        _ => return fail!("could not export the file into this format!"),
    }

    Ok(())
}
