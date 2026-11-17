use std::io::Write;

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
            DynamicValue::Zoned(zoned) => {
                self.scratch.clear();
                write!(&mut self.scratch, "{zoned}").unwrap();
                &self.scratch
            }
            DynamicValue::DateTime(datetime) => {
                self.scratch.clear();
                write!(&mut self.scratch, "{datetime}").unwrap();
                &self.scratch
            }
            DynamicValue::Date(date) => {
                self.scratch.clear();
                write!(&mut self.scratch, "{date}").unwrap();
                &self.scratch
            }
            DynamicValue::Time(time) => {
                self.scratch.clear();
                write!(&mut self.scratch, "{time}").unwrap();
                &self.scratch
            }
            DynamicValue::Span(span) => {
                self.scratch.clear();
                write!(&mut self.scratch, "{span}").unwrap();
                &self.scratch
            }
            DynamicValue::Regex(pattern) => pattern.as_str().as_bytes(),
            DynamicValue::None => b"",
        }
    }
}
