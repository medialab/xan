use std::fs::File;

use parquet::errors::Result as ParquetResult;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::Field;
use simd_csv::ByteRecord;

pub struct ParquetReader(SerializedFileReader<File>);

impl ParquetReader {
    pub fn new(file: File) -> ParquetResult<Self> {
        SerializedFileReader::new(file).map(Self)
    }

    pub fn byte_headers(&self) -> ByteRecord {
        let mut headers = ByteRecord::new();

        let schema = self.0.metadata().file_metadata().schema_descr();

        for column in schema.columns() {
            let path = column.path();
            let logical_name = &path.parts()[0];
            headers.push_field(logical_name.as_bytes());
        }

        headers
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
