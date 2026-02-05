use std::convert::TryFrom;
use std::num::NonZeroUsize;
use std::{
    fs,
    io::{self, BufReader, Cursor, Read},
    path::Path,
};

use calamine::{open_workbook_auto_from_rs, Data, Reader};
use flate2::read::MultiGzDecoder;
use jiff::civil::{DateTime, Time};
use serde_json::{Map, Value};
use simd_csv::ByteRecord;

use crate::config::Config;
use crate::json::JSONTabularizer;
use crate::util::{self, ChunksIteratorExt};
use crate::CliError;
use crate::CliResult;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
enum SupportedFormat {
    Xls,
    NdJSON,
    Json,
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
            "json" => Self::Json,
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
    - ods: OpenOffice spreadsheet
    - xls, xlsb, xlsx: Excel spreadsheet
    - json: JSON array or object
    - ndjson, jsonl: newline-delimited JSON data
    - txt: text lines
    - npy: numpy array
    - tar: tarball archive
    - md, markdown: Markdown table

Some formats can be streamed, some others require the full file to be loaded into
memory. The streamable formats are `ndjson`, `jsonl`, `tar`, `txt` and `npy`.

Some formats will handle gzip decompression on the fly if the filename ends
in `.gz`: `json`, `ndjson`, `jsonl`, `tar` and `txt`.

Tarball extraction was designed for utf8-encoded text files. Expect weird or
broken results with other encodings or binary files.

from options:
    -f, --format <format>  Format to convert from. Will be inferred from file
                           extension if not given. Must be specified when reading
                           from stdin, since we don't have a file extension to
                           work with.

Excel/OpenOffice-related options:
    --sheet-index <i>    0-based index of the sheet to convert. Defaults to converting
                         the first sheet. Use -s/--sheet alternatively to select a
                         sheet by name.
                         [default: 0]
    --sheet-name <name>  Name of the sheet to convert.
    --list-sheets        Print sheet names instead of converting file.

JSON options:
    --sample-size <n>      Number of records to sample before emitting headers.
                           Set to -1 to sample ALL records before emitting headers.
                           This may cost a lot of memory but will ensure all possible
                           keys have been observed and no data is lost when converting.
                           [default: 64]
    --sort-keys            Sort JSON keys lexicographically to emit columns accordingly.
                           This can be useful to harmonize different JSON sources with
                           no consistent key ordering.
    --key-column <name>    Name for the key column when parsing a JSON map.
                           [default: key]
    --value-column <name>  Name for the value column when parsing a JSON map.
                           [default: value]
    --single-object        Use if JSON only represents a single object that you want
                           to map to a single CSV row, instead of mapping to key,value columns.

Text lines options:
    -c, --column <name>    Name of the column to create.
                           [default: value]

Markdown options:
    -n, --nth-table <n>    Select nth table in document, starting at 0.
                           Negative index can be used to select from the end.
                           [default: 0]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_sheet_index: usize,
    flag_sheet_name: Option<String>,
    flag_list_sheets: bool,
    flag_format: Option<SupportedFormat>,
    flag_output: Option<String>,
    flag_sample_size: isize,
    flag_sort_keys: bool,
    flag_key_column: String,
    flag_value_column: String,
    flag_single_object: bool,
    flag_column: String,
    flag_nth_table: isize,
}

impl Args {
    fn writer(&self) -> io::Result<simd_csv::Writer<Box<dyn io::Write + Send>>> {
        Config::new(&self.flag_output).simd_writer()
    }

    fn sample_size(&self) -> Option<NonZeroUsize> {
        if self.flag_sample_size <= 0 {
            None
        } else {
            Some(NonZeroUsize::new(self.flag_sample_size as usize).unwrap())
        }
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

        let mut workbook = open_workbook_auto_from_rs(reader)?;

        if self.flag_list_sheets {
            let mut wtr = Config::new(&self.flag_output).io_writer()?;

            for sheet_name in workbook.sheet_names() {
                writeln!(&mut wtr, "{}", sheet_name)?;
            }

            return Ok(());
        }

        let mut wtr = self.writer()?;
        let mut record = ByteRecord::new();

        let range = match &self.flag_sheet_name {
            Some(name) => Some(workbook.worksheet_range(name)),
            None => workbook.worksheet_range_at(self.flag_sheet_index),
        };

        match range {
            None => {
                let sheets = workbook.sheet_names().len();

                Err(format!(
                    "--sheet-index {} is out-of-bounds (number of sheets: {})!",
                    self.flag_sheet_index, sheets
                ))?;
            }
            Some(Err(_)) => {
                let sheets = workbook.sheet_names().join(", ");

                Err(format!(
                    "could not find the \"{}\" sheet\nshould be one of: {}",
                    self.flag_sheet_name.as_ref().unwrap(),
                    sheets
                ))?;
            }
            Some(Ok(range)) => {
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
                            Data::DateTime(value) => {
                                let (year, month, day, hour, minute, second, millisecond) =
                                    value.to_ymd_hms_milli();

                                let datetime = DateTime::new(
                                    year as i16,
                                    month as i8,
                                    day as i8,
                                    hour as i8,
                                    minute as i8,
                                    second as i8,
                                    millisecond as i32 * 1_000_000,
                                )
                                .unwrap();

                                if datetime.time() == Time::MIN {
                                    record.push_field(datetime.date().to_string().as_bytes());
                                } else {
                                    record.push_field(datetime.to_string().as_bytes());
                                }
                            }
                            Data::Error(err) => record.push_field(err.to_string().as_bytes()),
                            Data::Empty => record.push_field(b""),
                        }
                    }

                    wtr.write_record(&record)?;
                }
            }
        }

        Ok(wtr.flush()?)
    }

    fn convert_ndjson(&self) -> CliResult<()> {
        use simd_json::Buffers;

        let mut buffers = Buffers::default();

        let wtr = self.writer()?;
        let mut rdr = simd_csv::LineReader::from_reader(Config::new(&self.arg_input).io_reader()?);
        let mut tabularizer = JSONTabularizer::from_writer(wtr, self.sample_size());

        if self.flag_sort_keys {
            tabularizer.reorder_keys();
        }

        while let Some(line) = rdr.read_line()? {
            // Sshhh... it's alright, really.
            let line_mut =
                unsafe { std::slice::from_raw_parts_mut(line.as_ptr() as *mut u8, line.len()) };

            let value: Value = simd_json::serde::from_slice_with_buffers(line_mut, &mut buffers)?;

            tabularizer.process(value)?;
        }

        Ok(tabularizer.flush()?)
    }

    fn convert_json(&self) -> CliResult<()> {
        let rdr = BufReader::new(Config::new(&self.arg_input).io_reader()?);

        let mut value =
            serde_json::from_reader(rdr).map_err(|err| CliError::Other(err.to_string()))?;

        // NOTE: recombobulating objects as collections
        if let Value::Object(object) = value {
            if self.flag_single_object {
                value = Value::Array(vec![Value::Object(object)]);
            } else {
                let mut items = Vec::with_capacity(object.len());

                for (k, v) in object {
                    items.push(Value::Object(Map::from_iter([
                        (self.flag_key_column.clone(), Value::String(k)),
                        (self.flag_value_column.clone(), v),
                    ])));
                }

                value = Value::Array(items);
            }
        }

        if let Value::Array(array) = value {
            let wtr = self.writer()?;
            let mut tabularizer = JSONTabularizer::from_writer(wtr, self.sample_size());

            if self.flag_sort_keys {
                tabularizer.reorder_keys();
            }

            for item in array.into_iter() {
                tabularizer.process(item)?;
            }

            Ok(tabularizer.flush()?)
        } else {
            Err(CliError::Other(
                "target JSON does not contain an array nor an object".to_string(),
            ))
        }
    }

    fn convert_text_lines(&self) -> CliResult<()> {
        let mut rdr = simd_csv::LineReader::from_reader(Config::new(&self.arg_input).io_reader()?);
        let mut wtr = self.writer()?;

        wtr.write_record([&self.flag_column])?;

        while let Some(line) = rdr.read_line()? {
            wtr.write_record([line])?;
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

        wtr.write_record_no_quoting((0..columns).map(|i| format!("dim_{}", i)))?;

        macro_rules! process {
            ($type: ty) => {
                for row in rdr
                    .data::<$type>()
                    .unwrap()
                    .chunks(NonZeroUsize::new(columns as usize).unwrap())
                {
                    // NOTE: this is safe because the output delimiter is guaranteed to be a comma
                    wtr.write_record_no_quoting(
                        row.into_iter().map(|cell| cell.unwrap().to_string()),
                    )?;
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
        let mut rdr: Box<dyn Read> = match self.arg_input.as_ref() {
            None => Box::new(io::stdin()),
            Some(p) => Box::new(fs::File::open(p)?),
        };

        if matches!(self.arg_input.as_ref(), Some(p) if p.ends_with(".gz")) {
            rdr = Box::new(MultiGzDecoder::new(rdr));
        }

        let mut archive = tar::Archive::new(rdr);

        let mut wtr = self.writer()?;

        let mut bytes: Vec<u8> = Vec::new();

        wtr.write_record(["path".as_bytes(), b"size", b"content"])?;

        for result in archive.entries()? {
            let mut entry = result?;

            if entry.size() == 0 {
                continue;
            }

            bytes.clear();

            if entry.path_bytes().ends_with(b".gz") {
                let mut inner_gz = MultiGzDecoder::new(&mut entry);
                inner_gz.read_to_end(&mut bytes)?;
            } else {
                entry.read_to_end(&mut bytes)?;
            }

            wtr.write_record([
                &entry.path_bytes(),
                entry.size().to_string().as_bytes(),
                &bytes,
            ])?;
        }

        Ok(wtr.flush()?)
    }

    fn convert_markdown(&self) -> CliResult<()> {
        use comrak::nodes::NodeValue;
        use comrak::{parse_document, Arena, Options};

        let mut rdr = Config::new(&self.arg_input).io_reader()?;
        let mut buf = String::new();
        rdr.read_to_string(&mut buf)?;

        let arena = Arena::new();
        let mut options = Options::default();
        options.extension.table = true;
        let root = parse_document(&arena, &buf, &options);
        let tables = root
            .descendants()
            .filter(|n| matches!(n.data.borrow().value, NodeValue::Table(_)))
            .collect::<Vec<_>>();

        if tables.is_empty() {
            Err("target Markdown does not contain a table")?;
        }
        let table = usize::try_from(self.flag_nth_table)
            .ok()
            // select from end if negative.
            .or_else(|| tables.len().checked_add_signed(self.flag_nth_table))
            .and_then(|i| tables.get(i))
            .ok_or_else(|| {
                let bounds = if self.flag_nth_table >= 0 {
                    [0, tables.len()].map(|n| n.to_string())
                } else {
                    // Saturating to avoid underflow.
                    // isize::MIN is smallest supported number anyway due to type of `flag_select`.
                    let low = 0isize.saturating_sub_unsigned(tables.len());
                    [-1, low].map(|n| n.to_string())
                };
                format!(
                    "table index {} is out of bounds in target Markdown (must be between {} and {})",
                    self.flag_nth_table,
                    bounds[0],
                    bounds[1]
                )
            })?;

        let mut wtr = self.writer()?;
        let mut record = ByteRecord::new();
        let lines = buf.lines().collect::<Vec<_>>();
        for row in table.children() {
            // Ignore non-row nodes, though there shouldn't be any.
            if !matches!(row.data.borrow().value, NodeValue::TableRow(_)) {
                continue;
            }
            for cell in row.children() {
                // Ignore non-cell nodes, though there shouldn't be any.
                if !matches!(cell.data.borrow().value, NodeValue::TableCell) {
                    continue;
                }

                // `cell.to_string()` drops formatting so extract raw string from `buf`.
                // Position of whole cell includes padding so get range from start
                // of first child to end of last child.
                if let Some((start, end)) = (|| {
                    let first = cell.first_child()?.data.borrow();
                    let last = cell.last_child()?.data.borrow();
                    Some((first.sourcepos.start, last.sourcepos.end))
                })() {
                    if start.line != end.line {
                        // Markdown does not support multiline table cells
                        // so this shouldn't happen.
                        return Err("Unsupported multiline Markdown table cell".into());
                    }
                    // sourcepos is 1-based, inclusive, and by bytes not characters
                    let line = lines[start.line - 1].as_bytes();
                    record.push_field(&line[start.column - 1..end.column]);
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
        SupportedFormat::Json => args.convert_json(),
        SupportedFormat::Text => args.convert_text_lines(),
        SupportedFormat::Npy => args.convert_npy(),
        SupportedFormat::Tar => args.convert_tar(),
        SupportedFormat::Md => args.convert_markdown(),
    }
}
