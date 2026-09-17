use std::fs::File;

use parquet::basic::{ConvertedType, LogicalType};
use parquet::errors::Result as ParquetResult;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::Field;
use parquet::schema::types::ColumnDescriptor;
use simd_csv::{ByteRecord, StringRecord};

pub struct ParquetReader(SerializedFileReader<File>);

impl ParquetReader {
    pub fn new(file: File) -> ParquetResult<Self> {
        SerializedFileReader::new(file).map(Self)
    }

    pub fn headers(&self) -> StringRecord {
        let mut headers = StringRecord::new();

        let schema = self.0.metadata().file_metadata().schema_descr();

        for column in schema.columns() {
            let path = column.path();
            let logical_name = &path.parts()[0];
            headers.push_field(logical_name);
        }

        headers
    }

    pub fn column_types(&self) -> Vec<&'static str> {
        let schema = self.0.metadata().file_metadata().schema_descr();

        schema
            .columns()
            .iter()
            .map(|column| human_readable_column_type(column))
            .collect()
    }

    pub fn byte_headers(&self) -> ByteRecord {
        self.headers().as_byte_record().clone()
    }

    pub fn count(&self) -> u64 {
        self.0.metadata().file_metadata().num_rows() as u64
    }

    pub fn into_inner(self) -> SerializedFileReader<File> {
        self.0
    }
}

pub fn push_parquet_field(record: &mut ByteRecord, field: &Field) -> Result<(), String> {
    match field {
        Field::Null => record.push_field(b""),
        Field::Bool(b) => record.push_field(if *b { b"true" } else { b"false" }),
        Field::Str(string) => record.push_field(string.as_bytes()),
        Field::Bytes(bytes) => record.push_field(bytes.data()),
        Field::UByte(f) => record.fmt_field(f),
        Field::UShort(f) => record.fmt_field(f),
        Field::UInt(f) => record.fmt_field(f),
        Field::ULong(f) => record.fmt_field(f),
        Field::Byte(f) => record.fmt_field(f),
        Field::Short(f) => record.fmt_field(f),
        Field::Int(f) => record.fmt_field(f),
        Field::Long(f) => record.fmt_field(f),
        Field::Float(f) => record.fmt_field(f),
        Field::Float16(f) => record.fmt_field(f),
        Field::Double(f) => record.fmt_field(f),
        Field::TimestampMicros(f) => record.fmt_field(f),
        Field::TimestampMillis(f) => record.fmt_field(f),
        Field::ListInternal(_) | Field::MapInternal(_) => record.write_field(|view| {
            serde_json::to_writer(view, &field.to_json_value()).unwrap();
        }),
        _ => Err("unsupported parquet value type!")?,
    };

    Ok(())
}

fn human_readable_column_type(column: &ColumnDescriptor) -> &'static str {
    if let Some(logical_type) = column.logical_type_ref() {
        return match logical_type {
            LogicalType::Timestamp(_) => "timestamp",
            _ => "unknown",
        };
    }

    match column.converted_type() {
        ConvertedType::UTF8 => "string",
        _ => "unknown",
    }
}
