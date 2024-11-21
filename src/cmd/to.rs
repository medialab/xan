use std::{
    fs,
    io::{self, Read, Write},
};

use csv;
use serde_json::{json, Value};

use crate::config::Config;
use crate::util;
use crate::CliError;
use crate::CliResult;

static USAGE: &str = "
Convert a CSV file to a variety of data formats.

Usage:
    xan to json [options] [<input>]
    xan to ndjson [options] [<input>]
    xan to jsonl [options] [<input>]
    xan to --help

Supported formats:
    json    - JSON array or object
    ndjson  - Newline-delimited JSON
    jsonl   - Newline-delimited JSON

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    cmd_json: bool,
    cmd_ndjson: bool,
    cmd_jsonl: bool,
    flag_output: Option<String>,
}

impl Args {
    fn convert_to_json<R: Read, W: Write>(mut rdr: csv::Reader<R>, writer: W) -> CliResult<()> {
        let headers = rdr.headers()?.clone();

        let mut json_array = Vec::new();

        for result in rdr.records() {
            let record = result?;
            let mut json_object = serde_json::Map::new();
            for (header, value) in headers.iter().zip(record.iter()) {
                if let Ok(parsed_value) = value.parse::<i64>() {
                    if parsed_value.abs() < 9007199254740991 {
                        json_object
                            .insert(header.to_string(), json!(value.parse::<f64>().unwrap()));
                        continue;
                    }
                }
                json_object.insert(header.to_string(), json!(value));
            }

            json_array.push(Value::Object(json_object));
        }
        let _ = serde_json::to_writer_pretty(writer, &json_array);

        Ok(())
    }

    fn convert_to_ndjson<R: Read>(mut rdr: csv::Reader<R>) -> CliResult<()> {
        let headers = rdr.headers()?.clone();

        for result in rdr.records() {
            let record = result?;
            let mut json_object = serde_json::Map::new();
            for (header, value) in headers.iter().zip(record.iter()) {
                if let Ok(parsed_value) = value.parse::<i64>() {
                    if parsed_value.abs() < 9007199254740991 {
                        json_object
                            .insert(header.to_string(), json!(value.parse::<f64>().unwrap()));
                        continue;
                    }
                }
                json_object.insert(header.to_string(), json!(value));
            }

            println!(
                "{}",
                serde_json::to_string(&json_object).map_err(|e| CliError::Other(e.to_string()))?
            );
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input);
    let rdr = conf.reader()?;

    if args.cmd_json {
        let writer: Box<dyn Write> = match &args.flag_output {
            Some(output_path) => Box::new(fs::File::create(output_path)?),
            None => Box::new(io::stdout()),
        };

        Args::convert_to_json(rdr, writer)?;
    } else if args.cmd_ndjson || args.cmd_jsonl {
        Args::convert_to_ndjson(rdr)?;
    }

    Ok(())
}
