use std::convert::TryFrom;
use std::num::NonZeroUsize;
use std::{
    fs,
    io::{self, BufRead, Cursor, Read},
    path::Path,
};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use flate2::read::GzDecoder;
use serde_json::{Map, Value};

use crate::config::Config;
use crate::json::for_each_json_value_as_csv_record;
use crate::util::{self, ChunksIteratorExt};
use crate::CliError;
use crate::CliResult;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
enum SupportedFormat {
    Xls,
    NdJSON,
    JSONArray,
    Text,
    Npy,
    Tar,
    Md,
}

impl SupportedFormat {
    fn parse(string: &str) -> Option<Self> {
        Some(match string {
            "xls" | "xlsx" | "xlsb" | "ods" => Self::Xls,
            "jsonl" | "ndjson" => Self::NdJSON,
            "json" => Self::JSONArray,
            "txt" | "text" | "lines" => Self::Text,
            "npy" => Self::Npy,
            "tar" | "tar.gz" => Self::Tar,
            "md" | "markdown" => Self::Md,
            _ => return None,
        })
    }

    fn infer_from_extension(path: &str) -> Option<Self> {
        let path = path.strip_suffix(".gz").unwrap_or(path);

        Self::parse(
            Path::new(path)
                .extension()
                .map(|e| e.to_str().unwrap())
                .unwrap_or(""),
        )
    }
}

impl TryFrom<String> for SupportedFormat {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SupportedFormat::parse(&value).ok_or_else(|| format!("unknown format \"{}\"", &value))
    }
}

static USAGE: &str = "
Convert a variety of data formats to CSV.

Usage:
    xan from [options] [<input>]
    xan from --help

Supported formats:
    ods      - OpenOffice spreadsheet
    xls      - Excel spreasheet
    xlsb     - Excel spreasheet
    xlsx     - Excel spreasheet
    json     - JSON array or object
    ndjson   - Newline-delimited JSON
    jsonl    - Newline-delimited JSON
    txt      - text lines
    npy      - Numpy array
    tar      - Tarball archive
    md       - Markdown table
    markdown - Markdown table

Some formats can be streamed, some others require the full file to be loaded into
memory. The streamable formats are `ndjson`, `jsonl`, `tar`, `txt` and `npy`.

Some formats will handle gzip decompression on the fly if the filename ends
in `.gz`: `json`, `ndjson`, `jsonl` and `txt`.

Tarball extraction was designed for utf8-encoded text files. Expect weird or
broken results with other encodings or binary files.

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
    --key-column <name>    Name for the key column when parsing a JSON map.
                           [default: key]
    --value-column <name>  Name for the value column when parsing a JSON map.
                           [default: value]

Text lines options:
    -c, --column <name>    Name of the column to create.
                           [default: value]

Markdown options:
    -s, --select <n>       Select nth table in document, starting at 0.
                           Negative index can be used to select from the end.
                           [default: 0]

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
    flag_key_column: String,
    flag_value_column: String,
    flag_column: String,
    flag_select: isize,
}

impl Args {
    fn writer(&self) -> io::Result<csv::Writer<Box<dyn io::Write + Send>>> {
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
        let rdr = Config::new(&self.arg_input).io_buf_reader()?;

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
        let mut rdr = Config::new(&self.arg_input).io_reader()?;

        let mut contents = String::new();
        rdr.read_to_string(&mut contents)?;

        let mut value =
            serde_json::from_str(&contents).map_err(|err| CliError::Other(err.to_string()))?;

        // NOTE: recombobulating objects as collections
        if let Value::Object(object) = value {
            let mut items = Vec::with_capacity(object.len());

            for (k, v) in object {
                items.push(Value::Object(Map::from_iter([
                    (self.flag_key_column.clone(), Value::String(k)),
                    (self.flag_value_column.clone(), v),
                ])));
            }

            value = Value::Array(items);
        }

        if let Value::Array(array) = value {
            let mut wtr = self.writer()?;

            for_each_json_value_as_csv_record(
                array.into_iter().map(Ok),
                self.flag_sample_size,
                |record| -> CliResult<()> {
                    wtr.write_record(record)?;
                    Ok(())
                },
            )?;

            Ok(wtr.flush()?)
        } else {
            Err(CliError::Other(
                "target JSON does not contain an array nor an object".to_string(),
            ))
        }
    }

    fn convert_text_lines(&self) -> CliResult<()> {
        let rdr = Config::new(&self.arg_input).io_buf_reader()?;
        let mut wtr = self.writer()?;
        wtr.write_record([&self.flag_column])?;

        let mut record = csv::ByteRecord::new();

        for result in rdr.lines() {
            let line = result?;
            let line = line.trim_end();

            if line.is_empty() {
                continue;
            }

            record.clear();
            record.push_field(line.as_bytes());

            wtr.write_byte_record(&record)?;
        }

        Ok(wtr.flush()?)
    }

    fn convert_npy(&self) -> CliResult<()> {
        use npyz::{DType, NpyFile, TypeChar};

        let rdr: Box<dyn Read> = match self.arg_input.as_ref() {
            None => Box::new(io::stdin()),
            Some(p) => Box::new(fs::File::open(p)?),
        };

        let rdr = NpyFile::new(rdr)?;

        let shape = rdr.shape();

        if shape.len() != 2 {
            Err("npy conversion only works with matrices (ndim = 2)!")?;
        }

        // let rows = shape[0];
        let columns = shape[1];

        let (type_char, size_field) = if let DType::Plain(descr) = rdr.dtype() {
            (descr.type_char(), descr.size_field())
        } else {
            return Err("npy conversion only works with simple dtypes!")?;
        };

        let mut wtr = self.writer()?;

        let mut record = csv::ByteRecord::new();

        for i in 0..columns {
            record.push_field(format!("dim_{}", i).as_bytes());
        }

        wtr.write_byte_record(&record)?;

        macro_rules! process {
            ($type: ty) => {
                for row in rdr
                    .data::<$type>()
                    .unwrap()
                    .chunks(NonZeroUsize::new(columns as usize).unwrap())
                {
                    record.clear();

                    for cell in row {
                        record.push_field(cell.unwrap().to_string().as_bytes());
                    }

                    wtr.write_byte_record(&record)?;
                }
            };
        }

        match (type_char, size_field) {
            (TypeChar::Float, 8) => {
                process!(f64)
            }
            (TypeChar::Float, 4) => {
                process!(f32)
            }
            _ => Err("unsupported dtype!")?,
        };

        Ok(wtr.flush()?)
    }

    fn convert_tar(&self) -> CliResult<()> {
        let rdr: Box<dyn Read> = match self.arg_input.as_ref() {
            None => Box::new(io::stdin()),
            Some(p) => Box::new(fs::File::open(p)?),
        };

        let rdr = GzDecoder::new(rdr);
        let mut archive = tar::Archive::new(rdr);

        let mut wtr = self.writer()?;

        let mut record = csv::ByteRecord::new();
        let mut bytes: Vec<u8> = Vec::new();

        record.push_field(b"path");
        record.push_field(b"size");
        record.push_field(b"content");

        wtr.write_byte_record(&record)?;

        for result in archive.entries()? {
            let mut entry = result?;

            if entry.size() == 0 {
                continue;
            }

            bytes.clear();

            if entry.path_bytes().ends_with(b".gz") {
                let mut inner_gz = GzDecoder::new(&mut entry);
                inner_gz.read_to_end(&mut bytes)?;
            } else {
                entry.read_to_end(&mut bytes)?;
            }

            record.clear();
            record.push_field(&entry.path_bytes());
            record.push_field(entry.size().to_string().as_bytes());
            record.push_field(&bytes);

            wtr.write_byte_record(&record)?;
        }

        Ok(wtr.flush()?)
    }

    fn convert_markdown(&self) -> CliResult<()> {
        use markdown::mdast::{Node, Table};

        let mut rdr = Config::new(&self.arg_input).io_buf_reader()?;
        let mut buf = String::new();
        rdr.read_to_string(&mut buf)?;

        // Use GitHub Flavored Markdown (GFM) for table support.
        let tree = markdown::to_mdast(&buf, &markdown::ParseOptions::gfm())?;

        fn collect_tables_into<'a>(n: &'a Node, tables: &mut Vec<&'a Table>) {
            if let Node::Table(table) = n {
                tables.push(table);
            } else {
                let Some(kids) = n.children() else { return };
                for n in kids {
                    collect_tables_into(n, tables);
                }
            }
        }
        let mut tables = vec![];
        collect_tables_into(&tree, &mut tables);
        if tables.is_empty() {
            Err("target Markdown does not contain a table")?;
        }
        let table = usize::try_from(self.flag_select)
            .ok()
            // select from end if negative.
            .or_else(|| tables.len().checked_add_signed(self.flag_select))
            .and_then(|i| tables.get(i))
            .ok_or_else(|| {
                let bounds = if self.flag_select >= 0 {
                    [0, tables.len()].map(|n| n.to_string())
                } else {
                    // Saturating to avoid underflow.
                    // isize::MIN is smallest supported number anyway due to type of `flag_select`. 
                    let low = 0isize.saturating_sub_unsigned(tables.len());
                    [-1, low].map(|n| n.to_string())
                };
                format!(
                    "table index {} is out of bounds in target Markdown (must be between {} and {})",
                    self.flag_select,
                    bounds[0],
                    bounds[1]
                )
            })?;

        let rows = &table.children;
        let mut wtr = self.writer()?;
        let mut record = csv::ByteRecord::new();
        for row in rows {
            for cell in row.children().into_iter().flatten() {
                // `cell.to_string()` drops formatting so extract raw string from `buf`.
                // Position of whole cell includes border `|` character so
                // get range from start of first child to end of last child.
                if let Some(range) = (|| {
                    let kids = cell.children()?;
                    let first = kids.first()?.position()?;
                    let last = kids.last()?.position()?;
                    Some(first.start.offset..last.end.offset)
                })() {
                    record.push_field(buf[range].as_bytes());
                } else {
                    record.push_field(&[]);
                }
            }
            wtr.write_byte_record(&record)?;
            record.clear();
        }

        Ok(wtr.flush()?)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let target_format = {
        if let Some(format) = args.flag_format {
            format
        } else if let Some(p) = args.arg_input.as_ref() {
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
                "cannot infer format from stdin. Please provide the -f/--format flag.".to_string(),
            ));
        }
    };

    match target_format {
        SupportedFormat::Xls => args.convert_xls(),
        SupportedFormat::NdJSON => args.convert_ndjson(),
        SupportedFormat::JSONArray => args.convert_json_array(),
        SupportedFormat::Text => args.convert_text_lines(),
        SupportedFormat::Npy => args.convert_npy(),
        SupportedFormat::Tar => args.convert_tar(),
        SupportedFormat::Md => args.convert_markdown(),
    }
}
