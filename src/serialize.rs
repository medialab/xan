use std::fmt::Display;
use std::io::Write;

use parquet::record::Field;

use crate::moonblade::DynamicValue;

#[derive(Default)]
pub struct AmortizedSerializer {
    scratch: Vec<u8>,
    int_buffer: itoa::Buffer,
    float_buffer: zmij::Buffer,
}

impl AmortizedSerializer {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    fn fmt<V: Display>(&mut self, value: V) -> &[u8] {
        self.scratch.clear();
        write!(&mut self.scratch, "{value}").unwrap();
        &self.scratch
    }

    pub fn serialize_value<'s, 'v>(&'s mut self, value: &'v DynamicValue) -> &'s [u8]
    where
        'v: 's,
    {
        match value {
            DynamicValue::List(_) | DynamicValue::Map(_) => {
                self.scratch.clear();
                serde_json::to_writer(&mut self.scratch, value).unwrap();
                &self.scratch
            }
            DynamicValue::String(string) => string.as_bytes(),
            DynamicValue::Bytes(bytes) => bytes,
            DynamicValue::Integer(i) => self.int_buffer.format(*i).as_bytes(),
            DynamicValue::Float(f) => self.float_buffer.format(*f).as_bytes(),
            DynamicValue::Boolean(v) => {
                if *v {
                    b"true"
                } else {
                    b"false"
                }
            }
            DynamicValue::Zoned(zoned) => self.fmt(zoned),
            DynamicValue::DateTime(datetime) => self.fmt(datetime),
            DynamicValue::Date(date) => self.fmt(date),
            DynamicValue::Time(time) => self.fmt(time),
            DynamicValue::Span(span) => self.fmt(span),
            DynamicValue::Regex(pattern) => pattern.as_str().as_bytes(),
            DynamicValue::None => b"",
        }
    }

    pub fn serialize_parquet_field<'s, 'v>(&'s mut self, field: &'v Field) -> &'s [u8]
    where
        'v: 's,
    {
        match field {
            Field::Null => b"",
            Field::Bool(b) => {
                if *b {
                    b"true"
                } else {
                    b"false"
                }
            }
            Field::Str(string) => string.as_bytes(),
            Field::Bytes(bytes) => bytes.data(),
            Field::UByte(i) => self.int_buffer.format(*i).as_bytes(),
            Field::UShort(i) => self.int_buffer.format(*i).as_bytes(),
            Field::UInt(i) => self.int_buffer.format(*i).as_bytes(),
            Field::ULong(i) => self.int_buffer.format(*i).as_bytes(),
            Field::Byte(i) => self.int_buffer.format(*i).as_bytes(),
            Field::Short(i) => self.int_buffer.format(*i).as_bytes(),
            Field::Int(i) => self.int_buffer.format(*i).as_bytes(),
            Field::Long(i) => self.int_buffer.format(*i).as_bytes(),
            Field::Float(f) => self.float_buffer.format(*f).as_bytes(),
            Field::Float16(f) => self.fmt(f),
            Field::Double(f) => self.float_buffer.format(*f).as_bytes(),
            Field::TimestampMicros(f) => self.fmt(f),
            Field::TimestampMillis(f) => self.fmt(f),
            Field::ListInternal(_) | Field::MapInternal(_) => {
                self.scratch.clear();
                serde_json::to_writer(&mut self.scratch, &field.to_json_value()).unwrap();
                &self.scratch
            }
            _ => panic!("unsupported parquet value type!"),
        }
    }
}
