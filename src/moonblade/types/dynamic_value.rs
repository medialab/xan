use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt;

use std::sync::Arc;

use bstr::BString;
use btoi::btoi;
use jiff::{
    civil::{Date, DateTime, Time},
    tz::TimeZone,
    Span, Zoned,
};
use regex::Regex;
use serde::{
    de::{Deserializer, MapAccess, SeqAccess, Visitor},
    Deserialize, Serialize, Serializer,
};
use url::Url;

use crate::temporal::{
    parse_any_temporal, parse_maybe_zoned, AnyTemporal, MaybeZoned, DEFAULT_DATETIME_PARSER,
};

use crate::collections::HashMap;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::utils::downgrade_float;
use crate::urls::TaggedUrl;

use super::DynamicNumber;

// NOTE: a DynamicValue should always be:
//   1. cheap to clone (notice the Arcs)
//   2. 24 bytes large max
#[derive(Debug, Clone, Default)]
pub enum DynamicValue {
    List(Arc<Vec<DynamicValue>>),
    Map(Arc<HashMap<String, DynamicValue>>),
    String(Arc<String>),
    Bytes(BString),
    Float(f64),
    Integer(i64),
    Boolean(bool),
    Regex(Arc<Regex>),
    Zoned(Box<Zoned>),
    DateTime(DateTime),
    Date(Date),
    Time(Time),
    Span(Box<Span>),
    #[default]
    None,
}

impl Serialize for DynamicValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Float(v) => v.serialize(serializer),
            Self::Integer(v) => v.serialize(serializer),
            Self::Boolean(v) => v.serialize(serializer),
            Self::String(v) => v.serialize(serializer),
            Self::Bytes(v) => v.serialize(serializer),
            Self::List(v) => v.serialize(serializer),
            Self::Map(v) => v.serialize(serializer),
            Self::Regex(v) => v.to_string().serialize(serializer),
            Self::Zoned(v) => v.to_string().serialize(serializer),
            Self::DateTime(v) => v.to_string().serialize(serializer),
            Self::Date(v) => v.to_string().serialize(serializer),
            Self::Time(v) => v.to_string().serialize(serializer),
            Self::Span(v) => v.to_string().serialize(serializer),
            Self::None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for DynamicValue {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<DynamicValue, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = DynamicValue;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("any valid JSON value")
            }

            #[inline]
            fn visit_bool<E>(self, value: bool) -> Result<DynamicValue, E> {
                Ok(DynamicValue::Boolean(value))
            }

            #[inline]
            fn visit_i64<E>(self, value: i64) -> Result<DynamicValue, E> {
                Ok(DynamicValue::Integer(value))
            }

            #[inline]
            fn visit_u64<E>(self, value: u64) -> Result<DynamicValue, E> {
                Ok(DynamicValue::Integer(value as i64))
            }

            #[inline]
            fn visit_f64<E>(self, value: f64) -> Result<DynamicValue, E> {
                Ok(DynamicValue::Float(value))
            }

            #[inline]
            fn visit_str<E>(self, value: &str) -> Result<DynamicValue, E>
            where
                E: serde::de::Error,
            {
                self.visit_string(String::from(value))
            }

            fn visit_string<E>(self, value: String) -> Result<DynamicValue, E> {
                Ok(DynamicValue::from(value))
            }

            #[inline]
            fn visit_none<E>(self) -> Result<DynamicValue, E> {
                Ok(DynamicValue::None)
            }

            #[inline]
            fn visit_some<D>(self, deserializer: D) -> Result<DynamicValue, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                Deserialize::deserialize(deserializer)
            }

            #[inline]
            fn visit_unit<E>(self) -> Result<DynamicValue, E> {
                Ok(DynamicValue::None)
            }

            #[inline]
            fn visit_seq<V>(self, mut visitor: V) -> Result<DynamicValue, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let mut vec = Vec::new();

                while let Ok(Some(elem)) = visitor.next_element() {
                    vec.push(elem);
                }

                Ok(DynamicValue::List(Arc::new(vec)))
            }

            fn visit_map<V>(self, mut visitor: V) -> Result<DynamicValue, V::Error>
            where
                V: MapAccess<'de>,
            {
                match visitor.next_key::<String>() {
                    Ok(Some(first_key)) => {
                        let mut map = HashMap::<String, DynamicValue>::new();
                        map.insert(first_key, visitor.next_value()?);

                        while let Ok(Some((key, value))) = visitor.next_entry() {
                            map.insert(key, value);
                        }

                        Ok(DynamicValue::Map(Arc::new(map)))
                    }
                    _ => Ok(DynamicValue::Map(Arc::new(HashMap::new()))),
                }
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

impl DynamicValue {
    pub fn from_owned_bytes(bytes: Vec<u8>) -> Self {
        Self::Bytes(BString::from(bytes))
    }

    pub fn empty_bytes() -> Self {
        Self::from_owned_bytes(b"".to_vec())
    }

    pub fn type_of(&self) -> &str {
        match self {
            Self::List(_) => "list",
            Self::Map(_) => "map",
            Self::String(_) => "string",
            Self::Bytes(_) => "bytes",
            Self::Float(_) => "float",
            Self::Integer(_) => "integer",
            Self::Boolean(_) => "boolean",
            Self::Zoned(_) => "zoned",
            Self::DateTime(_) => "datetime",
            Self::Date(_) => "date",
            Self::Time(_) => "time",
            Self::Span(_) => "span",
            Self::Regex(_) => "regex",
            Self::None => "none",
        }
    }

    fn is_scalar(&self) -> bool {
        !matches!(self, Self::List(_) | Self::Map(_))
    }

    pub fn serialize_as_bytes_with_options(&self, plural_separator: &[u8]) -> Cow<'_, [u8]> {
        match self {
            Self::List(list) => {
                if list.is_empty() {
                    return Cow::Borrowed(b"");
                }

                if list.iter().any(|v| !v.is_scalar()) {
                    return Cow::Owned(serde_json::to_string(self).unwrap().into_bytes());
                }

                if list.len() == 1 {
                    return list[0].serialize_as_bytes_with_options(plural_separator);
                }

                let mut bytes: Vec<u8> = Vec::new();

                for item in list.iter().take(list.len() - 1) {
                    bytes
                        .extend_from_slice(&item.serialize_as_bytes_with_options(plural_separator));
                    bytes.extend_from_slice(plural_separator);
                }

                bytes.extend_from_slice(
                    &list[list.len() - 1].serialize_as_bytes_with_options(plural_separator),
                );

                Cow::Owned(bytes)
            }
            Self::Map(_) => Cow::Owned(serde_json::to_string(self).unwrap().into_bytes()),
            Self::String(value) => Cow::Borrowed(value.as_bytes()),
            Self::Bytes(value) => Cow::Borrowed(value),
            Self::Float(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Integer(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Boolean(value) => Cow::Borrowed(if *value { b"true" } else { b"false" }),
            Self::Zoned(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::DateTime(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Date(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Time(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Span(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Regex(pattern) => Cow::Borrowed(pattern.as_str().as_bytes()),
            Self::None => Cow::Borrowed(b""),
        }
    }

    pub fn serialize_as_bytes(&self) -> Cow<'_, [u8]> {
        self.serialize_as_bytes_with_options(b"|")
    }

    pub fn try_as_timezone(&self) -> Result<TimeZone, EvaluationError> {
        let name = self.try_as_str()?;

        TimeZone::get(&name).map_err(|_| {
            EvaluationError::TimeRelated(format!("\"{}\" is not a valid timezone", name))
        })
    }

    pub fn try_into_maybe_zoned(self) -> Result<MaybeZoned, EvaluationError> {
        if let Self::Zoned(zoned) = self {
            return Ok(MaybeZoned::Zoned(*zoned));
        }

        if let Self::DateTime(datetime) = self {
            return Ok(MaybeZoned::Civil(datetime));
        }

        if self.is_temporal() {
            return Err(EvaluationError::from_cast(&self, "maybe_zoned"));
        }

        let bytes = self.try_as_bytes()?;

        match parse_maybe_zoned(bytes) {
            Err(_) => Err(EvaluationError::from_cast(&self, "maybe_zoned")),
            Ok(maybe) => Ok(maybe),
        }
    }

    pub fn try_as_any_temporal(&self) -> Result<AnyTemporal, EvaluationError> {
        Ok(match self {
            Self::Zoned(zoned) => AnyTemporal::Zoned(*zoned.clone()),
            Self::DateTime(datetime) => AnyTemporal::DateTime(*datetime),
            Self::Date(date) => AnyTemporal::Date(*date),
            Self::Time(time) => AnyTemporal::Time(*time),
            Self::String(string) => match parse_any_temporal(string.as_bytes()) {
                Ok(temporal) => temporal,
                Err(_) => {
                    return Err(EvaluationError::TimeRelated(format!(
                        "could not parse {} as a temporal value",
                        string
                    )))
                }
            },
            Self::Bytes(bytes) => match parse_any_temporal(bytes) {
                Ok(temporal) => temporal,
                Err(_) => {
                    return Err(EvaluationError::TimeRelated(format!(
                        "could not parse {} as a temporal value",
                        bytes
                    )))
                }
            },
            _ => return Err(EvaluationError::from_cast(self, "temporal")),
        })
    }

    pub fn try_into_zoned(self) -> Result<Zoned, EvaluationError> {
        if let Self::Zoned(zoned) = self {
            return Ok(*zoned);
        }

        let mismatch_err = || {
            EvaluationError::TimeRelated(format!(
                "this operation requires given datetime {:?} to have timezone information but it has none. You can use `with_timezone` or `with_local_timezone` to indicate it if you know the correct one beforehand.", self
            ))
        };

        match self {
            Self::DateTime(_) => return Err(mismatch_err()),
            Self::Date(date) => {
                return Err(EvaluationError::TimeRelated(format!(
                    "this operation cannot work on a bare date ({})",
                    date
                )))
            }
            Self::Time(time) => {
                return Err(EvaluationError::TimeRelated(format!(
                    "this operation cannot work on a bare time ({})",
                    time
                )))
            }
            _ => (),
        };

        let bytes = self.try_as_bytes()?;

        match parse_maybe_zoned(bytes) {
            Err(_) => Err(EvaluationError::from_cast(&self, "zoned")),
            Ok(maybe) => match maybe {
                MaybeZoned::Civil(_) => Err(mismatch_err()),
                MaybeZoned::Zoned(zoned) => Ok(zoned),
            },
        }
    }

    pub fn try_into_datetime(self) -> Result<DateTime, EvaluationError> {
        if let Self::DateTime(datetime) = self {
            return Ok(datetime);
        }

        if self.is_temporal() {
            return Err(EvaluationError::from_cast(&self, "datetime"));
        }

        let bytes = self.try_as_bytes()?;

        match parse_maybe_zoned(bytes) {
            Err(_) => Err(EvaluationError::from_cast(&self, "datetime")),
            Ok(maybe) => match maybe {
                MaybeZoned::Zoned(_) => Err(EvaluationError::from_cast(&self, "datetime")),
                MaybeZoned::Civil(datetime) => Ok(datetime),
            },
        }
    }

    pub fn try_into_date(self) -> Result<Date, EvaluationError> {
        if let Self::Date(date) = self {
            return Ok(date);
        }

        if self.is_temporal() {
            return Err(EvaluationError::from_cast(&self, "date"));
        }

        let bytes = self.try_as_bytes()?;

        match DEFAULT_DATETIME_PARSER.parse_date(bytes) {
            Err(_) => Err(EvaluationError::from_cast(&self, "date")),
            Ok(date) => Ok(date),
        }
    }

    pub fn try_into_time(self) -> Result<Time, EvaluationError> {
        if let Self::Time(time) = self {
            return Ok(time);
        }

        if self.is_temporal() {
            return Err(EvaluationError::from_cast(&self, "time"));
        }

        let bytes = self.try_as_bytes()?;

        match DEFAULT_DATETIME_PARSER.parse_time(bytes) {
            Err(_) => Err(EvaluationError::from_cast(&self, "time")),
            Ok(time) => Ok(time),
        }
    }

    pub fn try_as_tagged_url(&self) -> Result<TaggedUrl, EvaluationError> {
        self.try_as_str()?
            .parse::<TaggedUrl>()
            .map_err(|_| EvaluationError::from_cast(self, "url"))
    }

    pub fn try_as_url(&self) -> Result<Url, EvaluationError> {
        self.try_as_tagged_url()
            .map(|tagged_url| tagged_url.into_inner())
    }

    pub fn try_as_str(&self) -> Result<Cow<'_, str>, EvaluationError> {
        Ok(match self {
            Self::String(value) => Cow::Borrowed(value),
            Self::Bytes(value) => Cow::Borrowed(
                std::str::from_utf8(value).map_err(|_| EvaluationError::UnicodeDecodeError)?,
            ),
            Self::Float(value) => Cow::Owned(value.to_string()),
            Self::Integer(value) => Cow::Owned(value.to_string()),
            Self::Zoned(value) => Cow::Owned(value.to_string()),
            Self::Boolean(value) => Cow::Borrowed(if *value { "true" } else { "false" }),
            Self::Regex(pattern) => Cow::Borrowed(pattern.as_str()),
            Self::None => Cow::Borrowed(""),
            _ => return Err(EvaluationError::from_cast(self, "string")),
        })
    }

    pub fn try_as_bytes(&self) -> Result<&[u8], EvaluationError> {
        match self {
            Self::String(value) => Ok(value.as_bytes()),
            Self::Bytes(value) => Ok(value),
            _ => Err(EvaluationError::from_cast(self, "bytes")),
        }
    }

    pub fn try_as_regex(&self) -> Result<&Regex, EvaluationError> {
        match self {
            Self::Regex(regex) => Ok(regex),
            _ => Err(EvaluationError::from_cast(self, "regex")),
        }
    }

    pub fn try_as_list(&self) -> Result<&Vec<DynamicValue>, EvaluationError> {
        match self {
            Self::List(list) => Ok(list),
            _ => Err(EvaluationError::from_cast(self, "list")),
        }
    }

    pub fn try_into_arc_list(self) -> Result<Arc<Vec<DynamicValue>>, EvaluationError> {
        match self {
            Self::List(list) => Ok(list),
            _ => Err(EvaluationError::from_cast(&self, "list")),
        }
    }

    pub fn try_as_map(&self) -> Result<&HashMap<String, DynamicValue>, EvaluationError> {
        match self {
            Self::Map(map) => Ok(map),
            _ => Err(EvaluationError::from_cast(self, "map")),
        }
    }

    pub fn try_as_number(&self) -> Result<DynamicNumber, EvaluationError> {
        Ok(match self {
            Self::String(string) => match string.parse::<DynamicNumber>() {
                Err(_) => return Err(EvaluationError::from_cast(self, "number")),
                Ok(number) => number,
            },
            Self::Bytes(bytes) => match DynamicNumber::try_from(bytes.as_ref()) {
                Err(_) => return Err(EvaluationError::from_cast(self, "number")),
                Ok(number) => number,
            },
            Self::Integer(value) => DynamicNumber::Integer(*value),
            Self::Float(value) => DynamicNumber::Float(*value),
            Self::Boolean(value) => DynamicNumber::Integer(*value as i64),
            _ => return Err(EvaluationError::from_cast(self, "number")),
        })
    }

    pub fn try_as_usize(&self) -> Result<usize, EvaluationError> {
        Ok(match self {
            Self::String(string) => match string.parse::<usize>() {
                Err(_) => return Err(EvaluationError::from_cast(self, "unsigned_number")),
                Ok(value) => value,
            },
            Self::Bytes(bytes) => match btoi::<usize>(bytes) {
                Err(_) => return Err(EvaluationError::from_cast(self, "unsigned_number")),
                Ok(value) => value,
            },
            Self::Float(value) => match downgrade_float(*value) {
                Some(safe_downgraded_value) => {
                    if safe_downgraded_value >= 0 {
                        safe_downgraded_value as usize
                    } else {
                        return Err(EvaluationError::from_cast(self, "unsigned_number"));
                    }
                }
                None => return Err(EvaluationError::from_cast(self, "unsigned_number")),
            },
            Self::Integer(value) => {
                if value >= &0 {
                    (*value) as usize
                } else {
                    return Err(EvaluationError::from_cast(self, "unsigned_number"));
                }
            }
            Self::Boolean(value) => (*value) as usize,
            _ => return Err(EvaluationError::from_cast(self, "unsigned_number")),
        })
    }

    pub fn try_as_i64(&self) -> Result<i64, EvaluationError> {
        Ok(match self {
            Self::String(string) => match string.parse::<i64>() {
                Err(_) => return Err(EvaluationError::from_cast(self, "integer")),
                Ok(value) => value,
            },
            Self::Bytes(bytes) => match btoi::<i64>(bytes) {
                Err(_) => return Err(EvaluationError::from_cast(self, "integer")),
                Ok(value) => value,
            },
            Self::Float(value) => match downgrade_float(*value) {
                Some(safe_downgraded_value) => safe_downgraded_value,
                None => return Err(EvaluationError::from_cast(self, "integer")),
            },
            Self::Integer(value) => *value,
            Self::Boolean(value) => (*value) as i64,
            _ => return Err(EvaluationError::from_cast(self, "integer")),
        })
    }

    pub fn try_as_f64(&self) -> Result<f64, EvaluationError> {
        Ok(match self {
            Self::String(string) => match string.parse::<f64>() {
                Err(_) => return Err(EvaluationError::from_cast(self, "float")),
                Ok(value) => value,
            },
            Self::Bytes(bytes) => match fast_float::parse::<f64, &[u8]>(bytes.as_ref()) {
                Err(_) => return Err(EvaluationError::from_cast(self, "float")),
                Ok(value) => value,
            },
            Self::Float(value) => *value,
            Self::Integer(value) => *value as f64,
            Self::Boolean(value) => *value as usize as f64,
            _ => return Err(EvaluationError::from_cast(self, "float")),
        })
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::List(value) => !value.is_empty(),
            Self::Map(value) => !value.is_empty(),
            Self::String(value) => !value.is_empty(),
            Self::Bytes(value) => !value.is_empty(),
            Self::Float(value) => value == &0.0,
            Self::Integer(value) => value != &0,
            Self::Boolean(value) => *value,
            Self::Regex(pattern) => !pattern.as_str().is_empty(),
            Self::Zoned(_) | Self::DateTime(_) | Self::Date(_) | Self::Time(_) => true,
            Self::Span(span) => !span.is_zero(),
            Self::None => false,
        }
    }

    pub fn is_falsey(&self) -> bool {
        !self.is_truthy()
    }

    pub fn is_nullish(&self) -> bool {
        match self {
            Self::String(value) => value.is_empty(),
            Self::Bytes(value) => value.is_empty(),
            Self::None => true,
            _ => false,
        }
    }

    pub fn is_temporal(&self) -> bool {
        matches!(
            self,
            Self::Zoned(_) | Self::DateTime(_) | Self::Time(_) | Self::Date(_)
        )
    }

    pub fn flat_iter(&self) -> DynamicValueFlatIter<'_> {
        DynamicValueFlatIter::new(self)
    }

    pub fn set_bytes(&mut self, new_bytes: &[u8]) {
        match self {
            Self::Bytes(bytes) => {
                // NOTE: I cannot really prove this is faster to avoid allocation here...
                // It certainly seems a little bit faster but not by a large margin.
                bytes.clear();
                bytes.extend(new_bytes);
            }
            _ => panic!("DynamicValue is not Bytes!"),
        }
    }
}

pub struct DynamicValueFlatIter<'a> {
    queue: VecDeque<&'a DynamicValue>,
}

impl<'a> DynamicValueFlatIter<'a> {
    fn new(value: &'a DynamicValue) -> Self {
        let initial_capacity = match value {
            DynamicValue::List(list) => list.len(),
            _ => 1,
        };

        let mut queue: VecDeque<&DynamicValue> = VecDeque::with_capacity(initial_capacity);
        queue.push_back(value);

        DynamicValueFlatIter { queue }
    }
}

impl<'a> Iterator for DynamicValueFlatIter<'a> {
    type Item = &'a DynamicValue;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.queue.pop_front() {
                None => break None,
                Some(value) => match value {
                    DynamicValue::List(list) => {
                        for subvalue in list.iter().rev() {
                            self.queue.push_front(subvalue);
                        }

                        continue;
                    }
                    _ => break Some(value),
                },
            }
        }
    }
}

impl From<&[u8]> for DynamicValue {
    #[inline]
    fn from(value: &[u8]) -> Self {
        DynamicValue::from_owned_bytes(value.to_vec())
    }
}

impl From<&str> for DynamicValue {
    #[inline]
    fn from(value: &str) -> Self {
        DynamicValue::String(Arc::new(value.to_string()))
    }
}

impl From<Cow<'_, str>> for DynamicValue {
    #[inline]
    fn from(value: Cow<str>) -> Self {
        DynamicValue::String(Arc::new(value.into_owned()))
    }
}

impl From<String> for DynamicValue {
    #[inline]
    fn from(value: String) -> Self {
        DynamicValue::String(Arc::new(value))
    }
}

impl From<char> for DynamicValue {
    #[inline]
    fn from(value: char) -> Self {
        DynamicValue::String(Arc::new(value.to_string()))
    }
}

impl From<Regex> for DynamicValue {
    #[inline]
    fn from(value: Regex) -> Self {
        DynamicValue::Regex(Arc::new(value))
    }
}

impl From<Vec<DynamicValue>> for DynamicValue {
    #[inline]
    fn from(value: Vec<DynamicValue>) -> Self {
        DynamicValue::List(Arc::new(value))
    }
}

impl From<Arc<Vec<DynamicValue>>> for DynamicValue {
    #[inline]
    fn from(value: Arc<Vec<DynamicValue>>) -> Self {
        DynamicValue::List(value)
    }
}

impl From<HashMap<String, DynamicValue>> for DynamicValue {
    #[inline]
    fn from(value: HashMap<String, DynamicValue>) -> Self {
        DynamicValue::Map(Arc::new(value))
    }
}

impl From<Arc<HashMap<String, DynamicValue>>> for DynamicValue {
    #[inline]
    fn from(value: Arc<HashMap<String, DynamicValue>>) -> Self {
        DynamicValue::Map(value)
    }
}

impl From<bool> for DynamicValue {
    #[inline]
    fn from(value: bool) -> Self {
        DynamicValue::Boolean(value)
    }
}

impl From<usize> for DynamicValue {
    #[inline]
    fn from(value: usize) -> Self {
        DynamicValue::Integer(value as i64)
    }
}

impl From<i32> for DynamicValue {
    #[inline]
    fn from(value: i32) -> Self {
        DynamicValue::Integer(value as i64)
    }
}

impl From<i64> for DynamicValue {
    #[inline]
    fn from(value: i64) -> Self {
        DynamicValue::Integer(value)
    }
}

impl From<f64> for DynamicValue {
    #[inline]
    fn from(value: f64) -> Self {
        DynamicValue::Float(value)
    }
}

impl From<DynamicNumber> for DynamicValue {
    #[inline]
    fn from(value: DynamicNumber) -> Self {
        match value {
            DynamicNumber::Integer(value) => DynamicValue::Integer(value),
            DynamicNumber::Float(value) => DynamicValue::Float(value),
        }
    }
}

impl From<AnyTemporal> for DynamicValue {
    #[inline]
    fn from(value: AnyTemporal) -> Self {
        match value {
            AnyTemporal::Zoned(zoned) => Self::from(zoned),
            AnyTemporal::DateTime(datetime) => Self::from(datetime),
            AnyTemporal::Date(date) => Self::from(date),
            AnyTemporal::Time(time) => Self::from(time),
        }
    }
}

impl From<Zoned> for DynamicValue {
    #[inline]
    fn from(value: Zoned) -> Self {
        DynamicValue::Zoned(Box::new(value))
    }
}

impl From<DateTime> for DynamicValue {
    #[inline]
    fn from(value: DateTime) -> Self {
        DynamicValue::DateTime(value)
    }
}

impl From<Date> for DynamicValue {
    #[inline]
    fn from(value: Date) -> Self {
        DynamicValue::Date(value)
    }
}

impl From<Time> for DynamicValue {
    #[inline]
    fn from(value: Time) -> Self {
        DynamicValue::Time(value)
    }
}

impl From<Span> for DynamicValue {
    #[inline]
    fn from(value: Span) -> Self {
        DynamicValue::Span(Box::new(value))
    }
}

impl<T> From<Option<T>> for DynamicValue
where
    T: Into<DynamicValue>,
{
    #[inline]
    fn from(option: Option<T>) -> Self {
        match option {
            None => DynamicValue::None,
            Some(value) => value.into(),
        }
    }
}

impl PartialEq for DynamicValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Regex(a), Self::Regex(b)) => a.as_str() == b.as_str(),
            (Self::Boolean(a), Self::Boolean(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Zoned(a), Self::Zoned(b)) => a == b,
            (Self::DateTime(a), Self::DateTime(b)) => a == b,
            (Self::Date(a), Self::Date(b)) => a == b,
            (Self::Time(a), Self::Time(b)) => a == b,
            (Self::Span(a), Self::Span(b)) => a.fieldwise() == b.fieldwise(),
            (Self::None, Self::None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_value_flat_iter() {
        let integer = DynamicValue::Integer(3);
        let float = DynamicValue::Float(3.5);
        let string = DynamicValue::from("test");
        let list = DynamicValue::List(Arc::new(vec![
            DynamicValue::Integer(1),
            DynamicValue::Integer(2),
        ]));
        let recursive = DynamicValue::List(Arc::new(vec![
            DynamicValue::List(Arc::new(vec![
                DynamicValue::Integer(1),
                DynamicValue::Integer(2),
            ])),
            DynamicValue::Integer(3),
            DynamicValue::List(Arc::new(vec![DynamicValue::List(Arc::new(vec![
                DynamicValue::Integer(4),
            ]))])),
        ]));

        assert_eq!(integer.flat_iter().collect::<Vec<_>>(), vec![&integer]);
        assert_eq!(float.flat_iter().collect::<Vec<_>>(), vec![&float]);
        assert_eq!(string.flat_iter().collect::<Vec<_>>(), vec![&string]);
        assert_eq!(
            list.flat_iter().collect::<Vec<_>>(),
            vec![&DynamicValue::Integer(1), &DynamicValue::Integer(2)]
        );
        assert_eq!(
            recursive.flat_iter().collect::<Vec<_>>(),
            vec![
                &DynamicValue::Integer(1),
                &DynamicValue::Integer(2),
                &DynamicValue::Integer(3),
                &DynamicValue::Integer(4)
            ]
        );
    }
}
