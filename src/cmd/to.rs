use std::{
    fs,
    io::{self, Read, Write},
};

use csv::{self, StringRecord};
use serde_json::{json, Value};

use crate::config::Config;
use crate::util;
use crate::CliError;
use crate::CliResult;

static MAX_SAFE_INTEGER: i64 = 9007199254740991;
static USAGE: &str = "
Convert a CSV file to a variety of data formats.

Usage:
    xan to [<format>] [options] [<input>]
    xan to --help

Supported formats:
    json    - JSON array or object
    ndjson  - Newline-delimited JSON
    jsonl   - Newline-delimited JSON

to options:
    -E, --empty            Convert empty string to a null value.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
";

#[derive(Deserialize)]
struct Args {
    arg_format: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_empty: bool,
}

impl Args {
    fn make_json(
        &self,
        record: &mut StringRecord,
        headers: &StringRecord,
        mut json_object: serde_json::Map<String, Value>,
    ) -> serde_json::Map<String, Value> {
        for (header, value) in headers.iter().zip(record.iter()) {
            if let Ok(parsed_value) = value.parse::<i64>() {
                if parsed_value.abs() < MAX_SAFE_INTEGER {
                    json_object.insert(header.to_string(), json!(parsed_value as f64));
                    continue;
                }
            } else if let Ok(parsed_value) = value.parse::<f64>() {
                json_object.insert(header.to_string(), json!(parsed_value));
                continue;
            }
            if self.flag_empty && value == "" {
                json_object.insert(header.to_string(), json!(Value::Null));
                continue;
            }
            json_object.insert(header.to_string(), json!(value));
        }
        json_object
    }

    fn convert_to_json<R: Read, W: Write>(
        &self,
        mut rdr: csv::Reader<R>,
        writer: W,
    ) -> CliResult<()> {
        let headers = rdr.headers()?.clone();
        let mut record = csv::StringRecord::new();
        let mut json_object = serde_json::Map::new();

        let mut json_array = Vec::new();

        while rdr.read_record(&mut record)? {
            json_object = Args::make_json(&self, &mut record, &headers, json_object);

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

        while rdr.read_record(&mut record)? {
            json_object = Args::make_json(&self, &mut record, &headers, json_object);

            writeln!(
                writer,
                "{}",
                serde_json::to_string(&json_object).map_err(|e| CliError::Other(e.to_string()))?
            )?;
        }

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
        _ => return fail!("could not export the file into this format!"),
    }

    Ok(())
}
