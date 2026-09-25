use std::fs::File;

use parquet::basic::{ConvertedType, LogicalType, TimeUnit, Type};
use parquet::errors::Result as ParquetResult;
use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::record::Field;
use parquet::schema::types::{ColumnDescriptor, Type as SchemaType};
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

    pub fn project(&self, indices: &[usize]) -> ParquetResult<SchemaType> {
        let file_schema = self
            .0
            .metadata()
            .file_metadata()
            .schema_descr()
            .root_schema();

        let fields = indices
            .iter()
            .map(|&i| file_schema.get_fields()[i].clone())
            .collect::<Vec<_>>();

        let projection = SchemaType::group_type_builder(file_schema.name())
            .with_fields(fields)
            .build()?;

        Ok(projection)
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
        if !matches!(logical_type, LogicalType::Unknown) {
            return match logical_type {
                LogicalType::Timestamp(timestamp_type) => match timestamp_type.unit {
                    TimeUnit::MILLIS => "timestamp (ms)",
                    TimeUnit::MICROS => "timestamp (µs)",
                    TimeUnit::NANOS => "timestamp (ns)",
                },
                LogicalType::Map => "map",
                LogicalType::List => "list",
                LogicalType::String => "string",
                _ => "unknown",
            };
        }
    }

    match column.converted_type() {
        ConvertedType::UTF8 => "string",
        ConvertedType::UINT_8 => "uint8",
        ConvertedType::UINT_16 => "uint16",
        ConvertedType::UINT_32 => "uint32",
        ConvertedType::UINT_64 => "uint64",
        ConvertedType::INT_8 => "int8",
        ConvertedType::INT_16 => "int16",
        ConvertedType::INT_32 => "int32",
        ConvertedType::INT_64 => "int64",
        ConvertedType::MAP => "map",
        ConvertedType::LIST => "list",
        ConvertedType::TIMESTAMP_MILLIS => "timestamp (ms)",
        ConvertedType::TIMESTAMP_MICROS => "timestamp (µs)",
        ConvertedType::DECIMAL | ConvertedType::NONE => match column.physical_type() {
            Type::FLOAT => "float32",
            Type::DOUBLE => "float64",
            Type::INT32 => "int32",
            Type::INT64 => "int64",
            _ => "unknown",
        },
        _ => "unknown",
    }
}
