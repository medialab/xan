use std::borrow::Cow;
use std::cmp::{Ord, Ordering, PartialEq, PartialOrd};
use std::collections::{btree_map::Entry, BTreeMap, HashMap, VecDeque};
use std::convert::From;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, RangeInclusive, Rem, Sub};
use std::str::FromStr;
use std::sync::Arc;

use arrayvec::ArrayVec;
use csv::ByteRecord;
use jiff::{civil::DateTime, tz::TimeZone, Zoned};
use regex::Regex;
use serde::{
    de::{MapAccess, SeqAccess, Visitor},
    Deserialize, Serialize, Serializer,
};

use super::error::{ConcretizationError, EvaluationError, InvalidArity, SpecifiedEvaluationError};
use super::parser::Expr;
use super::utils::downgrade_float;

#[derive(Debug, PartialEq, Clone)]
pub enum ColumIndexationBy {
    Name(String),
    NameAndNth((String, usize)),
    Pos(usize),
}

impl ColumIndexationBy {
    pub fn from_arguments(arguments: &[&Expr]) -> Option<Self> {
        if arguments.len() == 1 {
            let first_arg = arguments.first().unwrap();
            match first_arg {
                Expr::Str(column_name) => Some(Self::Name(column_name.clone())),
                Expr::Float(_) | Expr::Int(_) => first_arg.try_to_usize().map(Self::Pos),
                _ => None,
            }
        } else if arguments.len() == 2 {
            match arguments.first().unwrap() {
                Expr::Str(column_name) => {
                    let second_arg = arguments.get(1).unwrap();

                    second_arg.try_to_usize().map(|column_index| {
                        Self::NameAndNth((column_name.to_string(), column_index))
                    })
                }
                _ => None,
            }
        } else {
            None
        }
    }

    pub fn from_argument(argument: &Expr) -> Option<Self> {
        Self::from_arguments(&[argument])
    }

    pub fn from_bound_arguments(
        name_or_pos: DynamicValue,
        pos: Option<DynamicValue>,
    ) -> Option<Self> {
        if let Some(pos_value) = pos {
            match pos_value.try_as_usize() {
                Err(_) => None,
                Ok(i) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::NameAndNth((name.into_owned(), i))),
                },
            }
        } else {
            match name_or_pos.try_as_usize() {
                Err(_) => match name_or_pos.try_as_str() {
                    Err(_) => None,
                    Ok(name) => Some(Self::Name(name.into_owned())),
                },
                Ok(i) => Some(Self::Pos(i)),
            }
        }
    }

    pub fn find_column_index(&self, headers: &ByteRecord) -> Option<usize> {
        match self {
            Self::Pos(i) => {
                if i >= &headers.len() {
                    None
                } else {
                    Some(*i)
                }
            }
            Self::Name(name) => {
                let name_bytes = name.as_bytes();

                for (i, cell) in headers.iter().enumerate() {
                    if cell == name_bytes {
                        return Some(i);
                    }
                }

                None
            }
            Self::NameAndNth((name, pos)) => {
                let mut c = *pos;

                let name_bytes = name.as_bytes();

                for (i, cell) in headers.iter().enumerate() {
                    if cell == name_bytes {
                        if c == 0 {
                            return Some(i);
                        }
                        c -= 1;
                    }
                }

                None
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct HeadersIndex {
    mapping: BTreeMap<String, Vec<usize>>,
}

impl HeadersIndex {
    pub fn new() -> Self {
        HeadersIndex {
            mapping: BTreeMap::new(),
        }
    }

    pub fn from_headers(headers: &ByteRecord) -> Self {
        let mut index = Self::new();

        for (i, header) in headers.iter().enumerate() {
            let key = std::str::from_utf8(header).unwrap().to_string();

            match index.mapping.entry(key) {
                Entry::Vacant(entry) => {
                    let positions: Vec<usize> = vec![i];
                    entry.insert(positions);
                }
                Entry::Occupied(mut entry) => {
                    entry.get_mut().push(i);
                }
            }
        }

        index
    }

    pub fn get(&self, indexation: &ColumIndexationBy) -> Option<usize> {
        match indexation {
            ColumIndexationBy::Name(name) => self
                .mapping
                .get(name)
                .and_then(|positions| positions.first())
                .copied(),
            ColumIndexationBy::Pos(pos) => {
                if *pos >= self.mapping.len() {
                    None
                } else {
                    Some(*pos)
                }
            }
            ColumIndexationBy::NameAndNth((name, pos)) => self
                .mapping
                .get(name)
                .and_then(|positions| positions.get(*pos))
                .copied(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Argument {
    Positional,
    Optional,
    Named(String),
}

impl Argument {
    pub fn with_name(name: &str) -> Self {
        Self::Named(name.to_string())
    }

    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Named(_) | Self::Optional)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionArguments {
    variadic: bool,
    arguments: Vec<Argument>,
}

impl FunctionArguments {
    pub fn nullary() -> Self {
        Self {
            variadic: false,
            arguments: Vec::new(),
        }
    }

    pub fn unary() -> Self {
        Self {
            variadic: false,
            arguments: vec![Argument::Positional],
        }
    }

    pub fn binary() -> Self {
        Self {
            variadic: false,
            arguments: vec![Argument::Positional; 2],
        }
    }

    pub fn nary(n: usize) -> Self {
        Self {
            variadic: false,
            arguments: vec![Argument::Positional; n],
        }
    }

    pub fn variadic(n: usize) -> Self {
        Self {
            variadic: true,
            arguments: vec![Argument::Positional; n],
        }
    }

    pub fn with_range(range: RangeInclusive<usize>) -> Self {
        let mut args = Vec::new();

        for _ in 0..*range.start() {
            args.push(Argument::Positional);
        }

        while args.len() < *range.end() {
            args.push(Argument::Optional);
        }

        Self {
            variadic: false,
            arguments: args,
        }
    }

    pub fn complex(arguments: Vec<Argument>) -> Self {
        Self {
            variadic: false,
            arguments,
        }
    }

    fn has_named(&self) -> bool {
        self.arguments
            .iter()
            .any(|arg| matches!(arg, Argument::Named(_)))
    }

    pub fn arity(&self) -> Arity {
        if self.variadic {
            Arity::Min(self.arguments.len())
        } else {
            let min = self
                .arguments
                .iter()
                .filter(|arg| !arg.is_optional())
                .count();

            if min == self.arguments.len() {
                Arity::Strict(min)
            } else {
                Arity::Range(min..=self.arguments.len())
            }
        }
    }

    pub fn validate_arity(&self, got: usize) -> Result<(), InvalidArity> {
        self.arity().validate(got)
    }

    fn find_named_arg_index(&self, name: &str) -> Option<usize> {
        self.arguments
            .iter()
            .position(|arg| matches!(arg, Argument::Named(n) if n == name))
    }

    // NOTE: arity will be validated beforehand
    pub fn reorder<T>(
        &self,
        actual_args: Vec<(Option<String>, T)>,
    ) -> Result<Vec<Option<T>>, ConcretizationError> {
        if self.variadic || !self.has_named() {
            return Ok(actual_args
                .into_iter()
                .map(|(_, value)| Some(value))
                .collect());
        }

        let mut reordered: Vec<Option<T>> = (0..self.arguments.len()).map(|_| None).collect();
        let mut i = 0;

        for (name_opt, value) in actual_args.into_iter() {
            match name_opt {
                Some(name) => {
                    match self.find_named_arg_index(&name) {
                        None => return Err(ConcretizationError::UnknownArgumentName(name)),
                        Some(j) => {
                            reordered[j] = Some(value);
                        }
                    };
                }
                None => {
                    reordered[i] = Some(value);
                    i += 1;
                }
            }
        }

        Ok(reordered)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Arity {
    Strict(usize),
    Min(usize),
    Range(RangeInclusive<usize>),
}

impl Arity {
    pub fn validate(self, got: usize) -> Result<(), InvalidArity> {
        match &self {
            Self::Strict(expected) => {
                if *expected != got {
                    Err(InvalidArity::from_arity(self, got))
                } else {
                    Ok(())
                }
            }
            Self::Min(expected_min) => {
                if got < *expected_min {
                    Err(InvalidArity::from_arity(self, got))
                } else {
                    Ok(())
                }
            }
            Self::Range(range) => {
                if !range.contains(&got) {
                    Err(InvalidArity::from_arity(self, got))
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DynamicNumber {
    Float(f64),
    Integer(i64),
}

impl DynamicNumber {
    pub fn abs(self) -> Self {
        match self {
            Self::Float(n) => Self::Float(n.abs()),
            Self::Integer(n) => Self::Integer(n.abs()),
        }
    }

    pub fn as_float(self) -> f64 {
        match self {
            Self::Float(f) => f,
            Self::Integer(i) => i as f64,
        }
    }

    pub fn idiv(self, rhs: Self) -> Self {
        Self::Integer(match self {
            Self::Integer(a) => match rhs {
                Self::Integer(b) => return Self::Integer(a / b),
                Self::Float(b) => (a as f64).div_euclid(b) as i64,
            },
            Self::Float(a) => match rhs {
                Self::Integer(b) => a.div_euclid(b as f64) as i64,
                Self::Float(b) => a.div_euclid(b) as i64,
            },
        })
    }

    pub fn pow(self, rhs: Self) -> Self {
        match rhs {
            Self::Integer(e) => match self {
                Self::Integer(n) => {
                    if e >= 0 && e <= u32::MAX as i64 {
                        DynamicNumber::Integer(n.pow(e as u32))
                    } else {
                        DynamicNumber::Float((n as f64).powf(e as f64))
                    }
                }
                Self::Float(n) => {
                    if e >= i32::MIN as i64 && e <= i32::MAX as i64 {
                        DynamicNumber::Float(n.powi(e as i32))
                    } else {
                        DynamicNumber::Float(n.powf(e as f64))
                    }
                }
            },
            Self::Float(e) => match self {
                DynamicNumber::Integer(n) => DynamicNumber::Float((n as f64).powf(e)),
                DynamicNumber::Float(n) => DynamicNumber::Float(n.powf(e)),
            },
        }
    }

    pub fn map_float<F>(self, callback: F) -> Self
    where
        F: Fn(f64) -> f64,
    {
        match self {
            Self::Integer(a) => Self::Float(callback(a as f64)),
            Self::Float(a) => Self::Float(callback(a)),
        }
    }

    pub fn map_float_to_int<F>(self, callback: F) -> Self
    where
        F: Fn(f64) -> f64,
    {
        match self {
            Self::Integer(_) => self,
            Self::Float(n) => Self::Integer(callback(n) as i64),
        }
    }

    pub fn floor(self) -> Self {
        self.map_float_to_int(|n| n.floor())
    }

    pub fn ceil(self) -> Self {
        self.map_float_to_int(|n| n.ceil())
    }

    pub fn trunc(self) -> Self {
        self.map_float_to_int(|n| n.trunc())
    }

    pub fn round(self) -> Self {
        self.map_float_to_int(|n| n.round())
    }

    pub fn ln(self) -> Self {
        self.map_float(|n| n.ln())
    }

    pub fn sqrt(self) -> Self {
        self.map_float(|n| n.sqrt())
    }
}

impl ToString for DynamicNumber {
    fn to_string(&self) -> String {
        match self {
            Self::Integer(n) => n.to_string(),
            Self::Float(n) => n.to_string(),
        }
    }
}

impl PartialEq for DynamicNumber {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Float(self_value) => match other {
                Self::Float(other_value) => self_value == other_value,
                Self::Integer(other_value) => *self_value == (*other_value as f64),
            },
            Self::Integer(self_value) => match other {
                Self::Float(other_value) => (*self_value as f64) == *other_value,
                Self::Integer(other_value) => self_value == other_value,
            },
        }
    }
}

impl Eq for DynamicNumber {}

impl PartialOrd for DynamicNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DynamicNumber {
    // TODO: NaN is gonna bite us in the buttocks at one point I'm sure..
    fn cmp(&self, other: &Self) -> Ordering {
        (match self {
            Self::Float(self_value) => match other {
                Self::Float(other_value) => self_value.partial_cmp(other_value),
                Self::Integer(other_value) => self_value.partial_cmp(&(*other_value as f64)),
            },
            Self::Integer(self_value) => match other {
                Self::Float(other_value) => (*self_value as f64).partial_cmp(other_value),
                Self::Integer(other_value) => Some(self_value.cmp(other_value)),
            },
        })
        .unwrap()
    }
}

fn apply_op<F1, F2>(
    lhs: DynamicNumber,
    rhs: DynamicNumber,
    op_int: F1,
    op_float: F2,
) -> DynamicNumber
where
    F1: FnOnce(i64, i64) -> i64,
    F2: FnOnce(f64, f64) -> f64,
{
    match lhs {
        DynamicNumber::Integer(a) => match rhs {
            DynamicNumber::Integer(b) => DynamicNumber::Integer(op_int(a, b)),
            DynamicNumber::Float(b) => DynamicNumber::Float(op_float(a as f64, b)),
        },
        DynamicNumber::Float(a) => match rhs {
            DynamicNumber::Integer(b) => DynamicNumber::Float(op_float(a, b as f64)),
            DynamicNumber::Float(b) => DynamicNumber::Float(op_float(a, b)),
        },
    }
}

impl Neg for DynamicNumber {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Float(v) => DynamicNumber::Float(-v),
            Self::Integer(v) => DynamicNumber::Integer(-v),
        }
    }
}

impl Rem for DynamicNumber {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Rem::<i64>::rem, Rem::<f64>::rem)
    }
}

impl Add for DynamicNumber {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Add::<i64>::add, Add::<f64>::add)
    }
}

impl AddAssign for DynamicNumber {
    fn add_assign(&mut self, rhs: Self) {
        match self {
            DynamicNumber::Float(a) => match rhs {
                DynamicNumber::Float(b) => *a += b,
                DynamicNumber::Integer(b) => *a += b as f64,
            },
            DynamicNumber::Integer(a) => match rhs {
                DynamicNumber::Float(b) => *self = DynamicNumber::Float((*a as f64) + b),
                DynamicNumber::Integer(b) => *a += b,
            },
        };
    }
}

impl Sub for DynamicNumber {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Sub::<i64>::sub, Sub::<f64>::sub)
    }
}

impl Mul for DynamicNumber {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Mul::<i64>::mul, Mul::<f64>::mul)
    }
}

impl Div for DynamicNumber {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        DynamicNumber::Float(match self {
            DynamicNumber::Integer(a) => match rhs {
                DynamicNumber::Integer(b) => a as f64 / b as f64,
                DynamicNumber::Float(b) => a as f64 / b,
            },
            DynamicNumber::Float(a) => match rhs {
                DynamicNumber::Integer(b) => a / b as f64,
                DynamicNumber::Float(b) => a / b,
            },
        })
    }
}

impl FromStr for DynamicNumber {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<i64>() {
            Err(_) => match s.parse::<f64>() {
                Err(_) => Err(()),
                Ok(n) => Ok(DynamicNumber::Float(n)),
            },
            Ok(n) => Ok(DynamicNumber::Integer(n)),
        }
    }
}

// NOTE: a DynamicValue should always be:
//   1. cheap to clone (notice the Arcs)
//   2. 24 bytes large max
#[derive(Debug, Clone)]
pub enum DynamicValue {
    List(Arc<Vec<DynamicValue>>),
    Map(Arc<HashMap<String, DynamicValue>>),
    String(String),
    Float(f64),
    Integer(i64),
    Boolean(bool),
    Regex(Arc<Regex>),
    DateTime(Box<Zoned>),
    None,
}

const DYNAMIC_VALUE_DATE_FORMAT: &str = "%FT%T[%Z]";

impl Default for DynamicValue {
    fn default() -> Self {
        Self::None
    }
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
            Self::List(v) => v.serialize(serializer),
            Self::Map(v) => v.serialize(serializer),
            Self::Regex(v) => v.to_string().serialize(serializer),
            Self::DateTime(v) => v
                .strftime(DYNAMIC_VALUE_DATE_FORMAT)
                .to_string()
                .serialize(serializer),
            Self::None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for DynamicValue {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<DynamicValue, D::Error>
    where
        D: serde::Deserializer<'de>,
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
                Ok(DynamicValue::String(value))
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
    pub fn type_of(&self) -> &str {
        match self {
            Self::List(_) => "list",
            Self::Map(_) => "map",
            Self::String(_) => "string",
            Self::Float(_) => "float",
            Self::Integer(_) => "integer",
            Self::Boolean(_) => "boolean",
            Self::DateTime(_) => "datetime",
            Self::Regex(_) => "regex",
            Self::None => "none",
        }
    }

    fn is_scalar(&self) -> bool {
        !matches!(self, Self::List(_) | Self::Map(_))
    }

    pub fn serialize_as_bytes_with_options(&self, plural_separator: &[u8]) -> Cow<[u8]> {
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
            Self::Float(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Integer(value) => Cow::Owned(value.to_string().into_bytes()),
            Self::Boolean(value) => Cow::Borrowed(if *value { b"true" } else { b"false" }),
            Self::DateTime(value) => Cow::Owned(
                value
                    .strftime(DYNAMIC_VALUE_DATE_FORMAT)
                    .to_string()
                    .into_bytes(),
            ),
            Self::Regex(pattern) => Cow::Borrowed(pattern.as_str().as_bytes()),
            Self::None => Cow::Borrowed(b""),
        }
    }

    pub fn serialize_as_bytes(&self) -> Cow<[u8]> {
        self.serialize_as_bytes_with_options(b"|")
    }

    pub fn try_into_datetime(self) -> Result<Box<Zoned>, EvaluationError> {
        match self {
            DynamicValue::DateTime(value) => Ok(value),
            DynamicValue::String(value) => match value.parse::<Zoned>() {
                Ok(zoned_datetime) => Ok(Box::new(zoned_datetime)),
                Err(_) => match value.parse::<DateTime>() {
                    Ok(datetime) => Ok(Box::new(datetime.to_zoned(TimeZone::system()).unwrap())),
                    Err(_) => Err(EvaluationError::DateTime(format!(
                        "cannot parse \"{}\" as a datetime, consider using datetime() with a custom format",
                        value
                    )))
                }
            },
            _ => Err(EvaluationError::from_cast(&self, "string"))
        }
    }

    pub fn try_as_str(&self) -> Result<Cow<str>, EvaluationError> {
        Ok(match self {
            Self::String(value) => Cow::Borrowed(value),
            Self::Float(value) => Cow::Owned(value.to_string()),
            Self::Integer(value) => Cow::Owned(value.to_string()),
            Self::DateTime(value) => Cow::Owned(value.to_string()),
            Self::Boolean(value) => Cow::Borrowed(if *value { "true" } else { "false" }),
            Self::Regex(pattern) => Cow::Borrowed(pattern.as_str()),
            Self::None => Cow::Borrowed(""),
            _ => return Err(EvaluationError::from_cast(self, "string")),
        })
    }

    pub fn try_into_string(self) -> Result<String, EvaluationError> {
        Ok(match self {
            Self::String(value) => value,
            Self::Float(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Boolean(value) => (if value { "true" } else { "false" }).to_string(),
            Self::Regex(pattern) => pattern.as_str().to_owned(),
            Self::None => "".to_string(),
            _ => return Err(EvaluationError::from_cast(&self, "string")),
        })
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
            Self::Float(value) => *value,
            Self::Integer(value) => *value as f64,
            Self::Boolean(value) => *value as usize as f64,
            _ => return Err(EvaluationError::from_cast(self, "float")),
        })
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::List(value) => !value.is_empty(),
            Self::Map(value) => !value.is_empty(),
            Self::String(value) => !value.is_empty(),
            Self::Float(value) => value == &0.0,
            Self::Integer(value) => value != &0,
            Self::Boolean(value) => *value,
            Self::Regex(pattern) => !pattern.as_str().is_empty(),
            Self::DateTime(_) => true,
            Self::None => false,
        }
    }

    pub fn is_falsey(&self) -> bool {
        !self.is_truthy()
    }

    pub fn is_nullish(&self) -> bool {
        match self {
            Self::String(value) => value.is_empty(),
            Self::None => true,
            _ => false,
        }
    }

    pub fn flat_iter(&self) -> DynamicValueFlatIter {
        DynamicValueFlatIter::new(self)
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

impl From<&str> for DynamicValue {
    fn from(value: &str) -> Self {
        DynamicValue::String(value.to_string())
    }
}

impl<'a> From<Cow<'a, str>> for DynamicValue {
    fn from(value: Cow<str>) -> Self {
        DynamicValue::String(value.into_owned())
    }
}

impl From<String> for DynamicValue {
    fn from(value: String) -> Self {
        DynamicValue::String(value)
    }
}

impl From<char> for DynamicValue {
    fn from(value: char) -> Self {
        DynamicValue::String(value.to_string())
    }
}

impl From<Regex> for DynamicValue {
    fn from(value: Regex) -> Self {
        DynamicValue::Regex(Arc::new(value))
    }
}

impl From<Vec<DynamicValue>> for DynamicValue {
    fn from(value: Vec<DynamicValue>) -> Self {
        DynamicValue::List(Arc::new(value))
    }
}

impl From<Arc<Vec<DynamicValue>>> for DynamicValue {
    fn from(value: Arc<Vec<DynamicValue>>) -> Self {
        DynamicValue::List(value)
    }
}

impl From<HashMap<String, DynamicValue>> for DynamicValue {
    fn from(value: HashMap<String, DynamicValue>) -> Self {
        DynamicValue::Map(Arc::new(value))
    }
}

impl From<Arc<HashMap<String, DynamicValue>>> for DynamicValue {
    fn from(value: Arc<HashMap<String, DynamicValue>>) -> Self {
        DynamicValue::Map(value)
    }
}

impl From<bool> for DynamicValue {
    fn from(value: bool) -> Self {
        DynamicValue::Boolean(value)
    }
}

impl From<usize> for DynamicValue {
    fn from(value: usize) -> Self {
        DynamicValue::Integer(value as i64)
    }
}

impl From<i32> for DynamicValue {
    fn from(value: i32) -> Self {
        DynamicValue::Integer(value as i64)
    }
}

impl From<i64> for DynamicValue {
    fn from(value: i64) -> Self {
        DynamicValue::Integer(value)
    }
}

impl From<f64> for DynamicValue {
    fn from(value: f64) -> Self {
        DynamicValue::Float(value)
    }
}

impl From<DynamicNumber> for DynamicValue {
    fn from(value: DynamicNumber) -> Self {
        match value {
            DynamicNumber::Integer(value) => DynamicValue::Integer(value),
            DynamicNumber::Float(value) => DynamicValue::Float(value),
        }
    }
}

impl From<Zoned> for DynamicValue {
    fn from(value: Zoned) -> Self {
        DynamicValue::DateTime(Box::new(value))
    }
}

impl<T> From<Option<T>> for DynamicValue
where
    T: Into<DynamicValue>,
{
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
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::DateTime(a), Self::DateTime(b)) => a == b,
            (Self::None, Self::None) => true,
            _ => false,
        }
    }
}

pub type EvaluationResult = Result<DynamicValue, SpecifiedEvaluationError>;

pub const BOUND_ARGUMENTS_CAPACITY: usize = 8;

pub struct BoundArguments {
    stack: ArrayVec<DynamicValue, BOUND_ARGUMENTS_CAPACITY>,
}

impl BoundArguments {
    pub fn new() -> Self {
        Self {
            stack: ArrayVec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn push(&mut self, arg: DynamicValue) {
        self.stack.push(arg);
    }

    pub fn get(&self, i: usize) -> Option<&DynamicValue> {
        self.stack.get(i)
    }

    pub fn get_not_none(&self, i: usize) -> Option<&DynamicValue> {
        let value = self.stack.get(i)?;

        match value {
            DynamicValue::None => None,
            _ => Some(value),
        }
    }

    pub fn get1(&self) -> &DynamicValue {
        &self.stack[0]
    }

    pub fn pop1(&mut self) -> DynamicValue {
        self.stack.pop().unwrap()
    }

    pub fn pop2(&mut self) -> (DynamicValue, DynamicValue) {
        let second = self.stack.pop().unwrap();
        let first = self.stack.pop().unwrap();

        (first, second)
    }

    pub fn pop3(&mut self) -> (DynamicValue, DynamicValue, DynamicValue) {
        let third = self.stack.pop().unwrap();
        let second = self.stack.pop().unwrap();
        let first = self.stack.pop().unwrap();

        (first, second, third)
    }

    pub fn get2(&self) -> (&DynamicValue, &DynamicValue) {
        (&self.stack[0], &self.stack[1])
    }

    pub fn get3(&self) -> (&DynamicValue, &DynamicValue, &DynamicValue) {
        (&self.stack[0], &self.stack[1], &self.stack[2])
    }

    pub fn get1_str(&self) -> Result<Cow<str>, EvaluationError> {
        self.get1().try_as_str()
    }

    pub fn pop1_bool(&mut self) -> bool {
        self.pop1().is_truthy()
    }

    pub fn pop1_number(&mut self) -> Result<DynamicNumber, EvaluationError> {
        self.pop1().try_as_number()
    }

    pub fn get2_str(&self) -> Result<(Cow<str>, Cow<str>), EvaluationError> {
        let (a, b) = self.get2();

        Ok((a.try_as_str()?, b.try_as_str()?))
    }

    pub fn get2_number(&self) -> Result<(DynamicNumber, DynamicNumber), EvaluationError> {
        let (a, b) = self.get2();

        Ok((a.try_as_number()?, b.try_as_number()?))
    }
}

pub struct BoundArgumentsIntoIterator(arrayvec::IntoIter<DynamicValue, BOUND_ARGUMENTS_CAPACITY>);

impl BoundArgumentsIntoIterator {
    pub fn next_not_none(&mut self) -> Option<DynamicValue> {
        self.next().and_then(|value| match value {
            DynamicValue::None => None,
            _ => Some(value),
        })
    }
}

impl Iterator for BoundArgumentsIntoIterator {
    type Item = DynamicValue;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl IntoIterator for BoundArguments {
    type Item = DynamicValue;
    type IntoIter = BoundArgumentsIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        BoundArgumentsIntoIterator(self.stack.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_value_flat_iter() {
        let integer = DynamicValue::Integer(3);
        let float = DynamicValue::Float(3.5);
        let string = DynamicValue::String("test".to_string());
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

    #[test]
    fn test_dynamic_number_ceil_floor_round() {
        assert_eq!(DynamicNumber::Float(2.3).ceil(), DynamicNumber::Integer(3));
        assert_eq!(DynamicNumber::Float(4.8).ceil(), DynamicNumber::Integer(5));
        assert_eq!(DynamicNumber::Integer(3).floor(), DynamicNumber::Integer(3));
        assert_eq!(DynamicNumber::Float(3.6).floor(), DynamicNumber::Integer(3));
        assert_eq!(
            DynamicNumber::Float(-3.6).floor(),
            DynamicNumber::Integer(-4)
        );
        assert_eq!(DynamicNumber::Integer(3).round(), DynamicNumber::Integer(3));
        assert_eq!(DynamicNumber::Float(3.6).round(), DynamicNumber::Integer(4));
        assert_eq!(DynamicNumber::Float(3.1).round(), DynamicNumber::Integer(3));
    }

    #[test]
    fn test_dynamic_number_ln_sqrt() {
        assert_eq!(DynamicNumber::Integer(1).ln(), DynamicNumber::Integer(0));
        assert_eq!(
            DynamicNumber::Float(3.5).ln(),
            DynamicNumber::Float(1.252762968495368)
        );
        assert_eq!(DynamicNumber::Integer(4).sqrt(), DynamicNumber::Integer(2));
        assert_eq!(
            DynamicNumber::Integer(100).sqrt(),
            DynamicNumber::Integer(10)
        );
    }

    #[test]
    fn test_dynamic_number_pow() {
        assert_eq!(
            DynamicNumber::Integer(2).pow(DynamicNumber::Integer(2)),
            DynamicNumber::Integer(4)
        );
    }
}
