use std::io::{self, IsTerminal, Write};
use std::iter;
use std::num::NonZeroUsize;

use npyz::WriterBuilder;
use pad::PadStr;
use rust_xlsxwriter::Workbook;
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::json::{JSONEmptyMode, JSONTypeInferrenceBuffer, OmittableAttributes};
use crate::select::SelectedColumns;
use crate::util;
use crate::xml::XMLWriter;
use crate::CliResult;

static USAGE: &str = "
Convert a CSV file to a variety of data formats.

Usage:
    xan to <format> [options] [<input>]
    xan to --help

Supported formats:
    html    - HTML table
    json    - JSON array or object
    jsonl   - JSON lines (same as `ndjson`)
    md      - Markdown table
    ndjson  - Newline-delimited JSON (same as `jsonl`)
    npy     - Numpy array
    txt     - Text lines
    xlsx    - Excel spreadsheet

Some formats can be streamed, some others require the full CSV file to be loaded into
memory.

Streamable formats are `html`, `jsonl`, `ndjson` and `txt`.

JSON options:
    -B, --buffer-size <size>  Number of CSV rows to sample to infer column types.
                              [default: 512]
    --nulls                   Convert empty string to a null value.
    --omit                    Ignore the empty values.

NPY options:
    --dtype <type>  Number type to use for the npy conversion. Must be one of \"f32\"
                    or \"f64\". [default: f64]

TXT options:
    -s, --select <column>  Column of file to emit as text. Will error if file
                           to convert to text has multiple columns or if
                           selection yields more than a single column.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be evaled
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_format: String,
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_select: SelectedColumns,
    flag_delimiter: Option<Delimiter>,
    flag_buffer_size: NonZeroUsize,
    flag_nulls: bool,
    flag_omit: bool,
    flag_dtype: String,
}

impl Args {
    fn is_writing_to_file(&self) -> bool {
        self.flag_output.is_some() || !io::stdout().is_terminal()
    }

    fn json_empty_mode(&self) -> JSONEmptyMode {
        if self.flag_nulls {
            JSONEmptyMode::Null
        } else if self.flag_omit {
            JSONEmptyMode::Omit
        } else {
            JSONEmptyMode::Empty
        }
    }

    fn rconf(&self) -> Config {
        Config::new(&self.arg_input)
            .no_headers(self.flag_no_headers)
            .delimiter(self.flag_delimiter)
    }

    fn wconf(&self) -> Config {
        Config::new(&self.flag_output)
    }

    fn convert_to_json(&self) -> CliResult<()> {
        let mut rdr = self.rconf().reader()?;
        let mut writer = self.wconf().buf_io_writer()?;

        let headers = rdr.headers()?.clone();

        let mut inferrence_buffer = JSONTypeInferrenceBuffer::with_columns(
            headers.len(),
            self.flag_buffer_size.get(),
            self.json_empty_mode(),
        );

        inferrence_buffer.read(&mut rdr)?;

        let mut json_object = OmittableAttributes::from_headers(headers.iter());
        let mut json_array = Vec::new();

        for record in inferrence_buffer.records() {
            inferrence_buffer.mutate_attributes(&mut json_object, record);
            json_array.push(json_object.clone());
        }

        let mut record = csv::StringRecord::new();

        while rdr.read_record(&mut record)? {
            inferrence_buffer.mutate_attributes(&mut json_object, &record);
            json_array.push(json_object.clone());
        }

        serde_json::to_writer_pretty(&mut writer, &json_array)?;
        writeln!(&mut writer)?;

        Ok(())
    }

    fn convert_to_ndjson(&self) -> CliResult<()> {
        let mut rdr = self.rconf().reader()?;
        let mut writer = self.wconf().buf_io_writer()?;

        let headers = rdr.headers()?.clone();
        let mut inferrence_buffer = JSONTypeInferrenceBuffer::with_columns(
            headers.len(),
            self.flag_buffer_size.get(),
            self.json_empty_mode(),
        );

        inferrence_buffer.read(&mut rdr)?;

        let mut json_object = OmittableAttributes::from_headers(headers.iter());

        for record in inferrence_buffer.records() {
            inferrence_buffer.mutate_attributes(&mut json_object, record);
            writeln!(writer, "{}", serde_json::to_string(&json_object)?)?;
        }

        let mut record = csv::StringRecord::new();

        while rdr.read_record(&mut record)? {
            inferrence_buffer.mutate_attributes(&mut json_object, &record);
            writeln!(writer, "{}", serde_json::to_string(&json_object)?)?;
        }

        Ok(())
    }

    fn convert_to_xlsx(&self) -> CliResult<()> {
        if !self.is_writing_to_file() {
            Err("cannot export in xlsx without a path.\nUse -o, --output or pipe the result!")?;
        }

        let mut rdr = self.rconf().reader()?;
        let mut writer = self.wconf().io_writer()?;

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

    fn convert_to_html(&self) -> CliResult<()> {
        let mut rdr = self.rconf().reader()?;
        let writer = self.wconf().buf_io_writer()?;

        let mut xml_writer = XMLWriter::new(writer);
        let mut record = csv::StringRecord::new();

        xml_writer.open_no_attributes("table")?;
        xml_writer.open_no_attributes("thead")?;
        xml_writer.open_no_attributes("tr")?;

        for header in rdr.headers()?.iter() {
            xml_writer.open_no_attributes("th")?;
            xml_writer.write_text(header)?;
            xml_writer.close("th")?;
        }

        xml_writer.close("tr")?;
        xml_writer.close("thead")?;

        xml_writer.open_no_attributes("tbody")?;

        while rdr.read_record(&mut record)? {
            xml_writer.open_no_attributes("tr")?;

            for cell in record.iter() {
                xml_writer.open_no_attributes("td")?;
                xml_writer.write_text(cell)?;
                xml_writer.close("td")?;
            }

            xml_writer.close("tr")?;
        }

        xml_writer.close("tbody")?;

        xml_writer.close("table")?;
        xml_writer.finish()?;

        Ok(())
    }

    fn convert_to_md(&self) -> CliResult<()> {
        let mut rdr = self.rconf().reader()?;
        let mut writer = self.wconf().buf_io_writer()?;

        fn escape_md_table_cell(cell: &str) -> String {
            cell.replace("|", "\\|")
                .replace("<", "\\<")
                .replace(">", "\\>")
        }

        let headers = rdr.headers()?.clone();
        let records = rdr
            .into_records()
            .map(|result| {
                result.map(|record| {
                    record
                        .into_iter()
                        .map(escape_md_table_cell)
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let widths = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                iter::once(h.width())
                    .chain(records.iter().map(move |r| r[i].width()))
                    .max()
                    .unwrap()
                    .max(3)
            })
            .collect::<Vec<_>>();

        write!(&mut writer, "|")?;

        for (header, width) in headers.iter().zip(widths.iter()) {
            write!(&mut writer, " {} |", header.pad_to_width(*width))?;
        }

        writeln!(&mut writer)?;

        write!(&mut writer, "|")?;

        for width in widths.iter().copied() {
            write!(&mut writer, " {} |", "-".repeat(width))?;
        }

        writeln!(&mut writer)?;

        for record in records.into_iter() {
            write!(&mut writer, "|")?;

            for (cell, width) in record.into_iter().zip(widths.iter()) {
                write!(&mut writer, " {} |", cell.pad_to_width(*width))?;
            }

            writeln!(&mut writer)?;
        }

        Ok(())
    }

    fn convert_to_npy(&self) -> CliResult<()> {
        if !self.is_writing_to_file() {
            Err("cannot export in npy without a path.\nUse -o, --output or pipe the result!")?;
        }

        let mut rdr = self.rconf().reader()?;
        let io_writer = self.wconf().io_writer()?;

        let records = rdr.byte_records().collect::<Result<Vec<_>, _>>()?;

        macro_rules! write_floats {
            ($type: ty) => {{
                let mut writer = npyz::WriteOptions::new()
                    .default_dtype()
                    .shape(&[records.len() as u64, rdr.byte_headers()?.len() as u64])
                    .writer(io_writer)
                    .begin_nd()?;

                for record in records.iter() {
                    for cell in record.iter() {
                        writer.push(
                            &fast_float::parse::<$type, &[u8]>(cell)
                                .map_err(|_| "could not parse some cell as dtype number!")?,
                        )?;
                    }
                }

                writer.finish()?;
            }};
        }

        match self.flag_dtype.as_str() {
            "float64" | "f64" => write_floats!(f64),
            "float32" | "f32" => write_floats!(f32),
            _ => Err(format!("unknown --dtype {}", self.flag_dtype))?,
        };

        Ok(())
    }

    fn convert_to_txt(&self) -> CliResult<()> {
        let mut rdr = self.rconf().simd_zero_copy_reader()?;
        let mut writer = self.wconf().buf_io_writer()?;

        let headers = rdr.byte_headers()?.clone();
        let column_index = self
            .flag_select
            .single_selection(&headers, rdr.has_headers()).map_err(|_| {
                "Trying to convert more than a single column to text!\nUse `xan select` upstream or use -s/--select flag to restrict column selection."
            })?;

        while let Some(record) = rdr.read_byte_record()? {
            let cell = record.unescape(column_index).unwrap();
            writer.write_all(&cell)?;
            writer.write_all(b"\n")?;
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    match args.arg_format.as_str() {
        "html" => args.convert_to_html(),
        "json" => args.convert_to_json(),
        "jsonl" | "ndjson" => args.convert_to_ndjson(),
        "md" => args.convert_to_md(),
        "npy" => args.convert_to_npy(),
        "txt" | "text" => args.convert_to_txt(),
        "xlsx" => args.convert_to_xlsx(),
        _ => Err("could not export the file to this format!")?,
    }
}
