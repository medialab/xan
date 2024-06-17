use std::borrow::Cow;
use std::cmp::max;
use std::cmp::{Ordering, PartialOrd};
use std::fs::{self, File};
use std::io::Read;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};
use std::path::Path;
use std::path::PathBuf;

use bytesize::ByteSize;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use flate2::read::GzDecoder;
use unidecode::unidecode;
use uuid::Uuid;

use super::error::EvaluationError;
use super::types::{Arity, BoundArguments, DynamicNumber, DynamicValue};

type FunctionResult = Result<DynamicValue, EvaluationError>;
pub type Function = fn(BoundArguments) -> FunctionResult;

pub fn get_function(name: &str) -> Option<(Function, Arity)> {
    Some(match name {
        "__num_eq" => (
            |args| number_compare(args, Ordering::is_eq),
            Arity::Strict(2),
        ),
        "__num_gt" => (
            |args| number_compare(args, Ordering::is_gt),
            Arity::Strict(2),
        ),
        "__num_ge" => (
            |args| number_compare(args, Ordering::is_ge),
            Arity::Strict(2),
        ),
        "__num_lt" => (
            |args| number_compare(args, Ordering::is_lt),
            Arity::Strict(2),
        ),
        "__num_le" => (
            |args| number_compare(args, Ordering::is_le),
            Arity::Strict(2),
        ),
        "__num_ne" => (
            |args| number_compare(args, Ordering::is_ne),
            Arity::Strict(2),
        ),
        "abs" => (
            |args| unary_arithmetic_op(args, DynamicNumber::abs),
            Arity::Strict(1),
        ),
        "abspath" => (abspath, Arity::Strict(1)),
        "add" => (|args| variadic_arithmetic_op(args, Add::add), Arity::Min(2)),
        "and" => (and, Arity::Min(2)),
        "argmin" => (argmin, Arity::Range(1..=2)),
        "bytesize" => (bytesize, Arity::Strict(1)),
        "ceil" => (
            |args| unary_arithmetic_op(args, DynamicNumber::ceil),
            Arity::Strict(1),
        ),
        "coalesce" => (coalesce, Arity::Min(1)),
        "compact" => (compact, Arity::Strict(1)),
        "concat" => (concat, Arity::Min(1)),
        "contains" => (contains, Arity::Strict(2)),
        "count" => (count, Arity::Strict(2)),
        "div" => (|args| variadic_arithmetic_op(args, Div::div), Arity::Min(2)),
        "endswith" => (endswith, Arity::Strict(2)),
        "err" => (err, Arity::Strict(1)),
        "escape_regex" => (escape_regex, Arity::Strict(1)),
        "ext" => (ext, Arity::Strict(1)),
        "filesize" => (filesize, Arity::Strict(1)),
        "first" => (first, Arity::Strict(1)),
        "floor" => (
            |args| unary_arithmetic_op(args, DynamicNumber::floor),
            Arity::Strict(1),
        ),
        "fmt" => (fmt, Arity::Min(1)),
        "get" => (get, Arity::Range(2..=3)),
        "idiv" => (
            |args| arithmetic_op(args, DynamicNumber::idiv),
            Arity::Strict(2),
        ),
        "isfile" => (isfile, Arity::Strict(1)),
        "join" => (join, Arity::Strict(2)),
        "json_parse" => (json_parse, Arity::Strict(1)),
        "last" => (last, Arity::Strict(1)),
        "len" => (len, Arity::Strict(1)),
        "log" => (
            |args| unary_arithmetic_op(args, DynamicNumber::ln),
            Arity::Strict(1),
        ),
        "ltrim" => (ltrim, Arity::Range(1..=2)),
        "lower" => (lower, Arity::Strict(1)),
        "max" => (variadic_max, Arity::Min(2)),
        "md5" => (md5, Arity::Strict(1)),
        "min" => (variadic_min, Arity::Min(2)),
        "mod" => (
            |args| binary_arithmetic_op(args, Rem::rem),
            Arity::Strict(2),
        ),
        "mul" => (|args| variadic_arithmetic_op(args, Mul::mul), Arity::Min(2)),
        "neg" => (|args| unary_arithmetic_op(args, Neg::neg), Arity::Strict(1)),
        "not" => (not, Arity::Strict(1)),
        "or" => (or, Arity::Min(2)),
        "pathjoin" => (pathjoin, Arity::Min(1)),
        "pow" => (
            |args| binary_arithmetic_op(args, DynamicNumber::pow),
            Arity::Strict(2),
        ),
        "read" => (read, Arity::Range(1..=3)),
        "replace" => (replace, Arity::Strict(3)),
        "round" => (
            |args| unary_arithmetic_op(args, DynamicNumber::round),
            Arity::Strict(1),
        ),
        "rtrim" => (rtrim, Arity::Range(1..=2)),
        "slice" => (slice, Arity::Range(2..=3)),
        "split" => (split, Arity::Range(2..=3)),
        "sqrt" => (
            |args| unary_arithmetic_op(args, DynamicNumber::sqrt),
            Arity::Strict(1),
        ),
        "startswith" => (startswith, Arity::Strict(2)),
        "sub" => (|args| variadic_arithmetic_op(args, Sub::sub), Arity::Min(2)),
        "eq" => (
            |args| sequence_compare(args, Ordering::is_eq),
            Arity::Strict(2),
        ),
        "gt" => (
            |args| sequence_compare(args, Ordering::is_gt),
            Arity::Strict(2),
        ),
        "ge" => (
            |args| sequence_compare(args, Ordering::is_ge),
            Arity::Strict(2),
        ),
        "lt" => (
            |args| sequence_compare(args, Ordering::is_lt),
            Arity::Strict(2),
        ),
        "le" => (
            |args| sequence_compare(args, Ordering::is_le),
            Arity::Strict(2),
        ),
        "ne" => (
            |args| sequence_compare(args, Ordering::is_ne),
            Arity::Strict(2),
        ),
        "trim" => (trim, Arity::Range(1..=2)),
        "trunc" => (
            |args| unary_arithmetic_op(args, DynamicNumber::trunc),
            Arity::Strict(1),
        ),
        "typeof" => (type_of, Arity::Strict(1)),
        "unidecode" => (apply_unidecode, Arity::Strict(1)),
        "upper" => (upper, Arity::Strict(1)),
        "uuid" => (uuid, Arity::Strict(0)),
        "val" => (val, Arity::Strict(1)),
        "write" => (write, Arity::Strict(2)),
        _ => return None,
    })
}

// Strings
fn trim(args: BoundArguments) -> FunctionResult {
    let string = args.get(0).unwrap().try_as_str()?;
    let arg2 = args.get(1);

    Ok(match arg2 {
        None => DynamicValue::from(string.trim()),
        Some(arg) => {
            let pattern = arg.try_as_str()?.chars().collect::<Vec<char>>();
            DynamicValue::from(string.trim_matches(|c| pattern.contains(&c)))
        }
    })
}

fn ltrim(args: BoundArguments) -> FunctionResult {
    let string = args.get(0).unwrap().try_as_str()?;
    let arg2 = args.get(1);

    Ok(match arg2 {
        None => DynamicValue::from(string.trim_start()),
        Some(arg) => {
            let pattern = arg.try_as_str()?.chars().collect::<Vec<char>>();
            DynamicValue::from(string.trim_start_matches(|c| pattern.contains(&c)))
        }
    })
}

fn rtrim(args: BoundArguments) -> FunctionResult {
    let string = args.get(0).unwrap().try_as_str()?;
    let arg2 = args.get(1);

    Ok(match arg2 {
        None => DynamicValue::from(string.trim_end()),
        Some(arg) => {
            let pattern = arg.try_as_str()?.chars().collect::<Vec<char>>();
            DynamicValue::from(string.trim_end_matches(|c| pattern.contains(&c)))
        }
    })
}

fn escape_regex(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(regex::escape(args.get1_str()?.as_ref())))
}

fn md5(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(format!(
        "{:x}",
        md5::compute(args.get1_str()?.as_bytes())
    )))
}

fn split(args: BoundArguments) -> FunctionResult {
    let args = args.getn_opt(3);

    let to_split = args[0].unwrap().try_as_str()?;
    let pattern = args[1].unwrap().try_as_str()?;
    let count = args[2];

    let splitted: Vec<DynamicValue> = if let Some(c) = count {
        to_split
            .splitn(c.try_as_usize()? + 1, &*pattern)
            .map(DynamicValue::from)
            .collect()
    } else {
        to_split.split(&*pattern).map(DynamicValue::from).collect()
    };

    Ok(DynamicValue::from(splitted))
}

fn lower(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(args.get1_str()?.to_lowercase()))
}

fn upper(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(args.get1_str()?.to_uppercase()))
}

fn len(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(DynamicValue::from(match arg.as_ref() {
        DynamicValue::List(list) => list.len(),
        _ => arg.try_as_str()?.len(),
    }))
}

fn count(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    let string = arg1.try_as_str()?;

    match arg2.try_as_regex() {
        Ok(regex) => Ok(DynamicValue::from(regex.find_iter(&string).count())),
        Err(_) => {
            let pattern = arg2.try_as_str()?;

            Ok(DynamicValue::from(string.matches(pattern.as_ref()).count()))
        }
    }
}

fn startswith(args: BoundArguments) -> FunctionResult {
    let (string, pattern) = args.get2_str()?;

    Ok(DynamicValue::from(string.starts_with(&*pattern)))
}

fn endswith(args: BoundArguments) -> FunctionResult {
    let (string, pattern) = args.get2_str()?;

    Ok(DynamicValue::from(string.ends_with(&*pattern)))
}

fn concat(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let first = args_iter.next().unwrap();

    match first.as_ref() {
        DynamicValue::List(list) => {
            let mut result: Vec<DynamicValue> = list.clone();

            for arg in args_iter {
                result.push(arg.as_ref().clone());
            }

            Ok(DynamicValue::List(result))
        }
        value => {
            let mut result = String::new();
            result.push_str(&value.try_as_str()?);

            for arg in args_iter {
                result.push_str(&arg.try_as_str()?);
            }

            Ok(DynamicValue::from(result))
        }
    }
}

fn apply_unidecode(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;

    Ok(DynamicValue::from(unidecode(&arg)))
}

lazy_static! {
    static ref FMT_PATTERN: regex::Regex = regex::Regex::new(r"\{\}").unwrap();
}

fn fmt(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let first_arg = args_iter.next().unwrap();
    let pattern = first_arg.try_as_str()?;

    let mut formatted = String::with_capacity(pattern.len());
    let mut last_match = 0;

    for capture in FMT_PATTERN.captures_iter(&pattern) {
        let m = capture.get(0).unwrap();

        formatted.push_str(&pattern[last_match..m.start()]);

        match args_iter.next() {
            None => formatted.push_str(&capture[0]),
            Some(arg) => {
                formatted.push_str(&arg.try_as_str()?);
            }
        }

        last_match = m.end();
    }

    formatted.push_str(&pattern[last_match..]);

    Ok(DynamicValue::from(formatted))
}

// Lists & Sequences
fn first(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(match arg {
        Cow::Borrowed(value) => match value {
            DynamicValue::String(string) => DynamicValue::from(string.chars().next()),
            DynamicValue::List(list) => match list.first() {
                None => DynamicValue::None,
                Some(value) => value.clone(),
            },
            _ => {
                return Err(EvaluationError::Cast((
                    value.type_of().to_string(),
                    "sequence".to_string(),
                )))
            }
        },
        Cow::Owned(value) => match value {
            DynamicValue::String(string) => DynamicValue::from(string.chars().next()),
            DynamicValue::List(mut list) => {
                if list.is_empty() {
                    DynamicValue::None
                } else {
                    list.swap_remove(0)
                }
            }
            _ => {
                return Err(EvaluationError::Cast((
                    value.type_of().to_string(),
                    "sequence".to_string(),
                )))
            }
        },
    })
}

fn last(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(match arg {
        Cow::Borrowed(value) => match value {
            DynamicValue::String(string) => DynamicValue::from(string.chars().next_back()),
            DynamicValue::List(list) => match list.last() {
                None => DynamicValue::None,
                Some(value) => value.clone(),
            },
            _ => {
                return Err(EvaluationError::Cast((
                    value.type_of().to_string(),
                    "sequence".to_string(),
                )))
            }
        },
        Cow::Owned(value) => match value {
            DynamicValue::String(string) => DynamicValue::from(string.chars().next_back()),
            DynamicValue::List(mut list) => match list.pop() {
                None => DynamicValue::None,
                Some(value) => value,
            },
            _ => {
                return Err(EvaluationError::Cast((
                    value.type_of().to_string(),
                    "sequence".to_string(),
                )))
            }
        },
    })
}

fn get(args: BoundArguments) -> FunctionResult {
    let (target, key) = args.get2();

    let default = || {
        args.get(2)
            .map(|v| v.as_ref().clone())
            .unwrap_or_else(|| DynamicValue::None)
    };

    Ok(match target.as_ref() {
        DynamicValue::String(value) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += value.len() as i64;
            }

            if index < 0 {
                default()
            } else {
                match value.chars().nth(index as usize) {
                    Some(c) => DynamicValue::from(c),
                    None => default(),
                }
            }
        }
        DynamicValue::List(list) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += list.len() as i64;
            }

            if index < 0 {
                default()
            } else {
                match list.get(index as usize) {
                    None => default(),
                    Some(value) => value.clone(),
                }
            }
        }
        DynamicValue::Map(map) => {
            let key = key.try_as_str()?;

            match map.get(key.as_ref()) {
                None => default(),
                Some(value) => value.clone(),
            }
        }
        value => {
            return Err(EvaluationError::Cast((
                value.type_of().to_string(),
                "sequence".to_string(),
            )))
        }
    })
}

fn slice(args: BoundArguments) -> FunctionResult {
    let args = args.getn_opt(3);

    let target = args[0].unwrap();

    match target.as_ref() {
        DynamicValue::String(string) => {
            let mut lo = args[1].unwrap().try_as_i64()?;
            let opt_hi = args[2];

            let chars = string.chars();

            let substring: String = match opt_hi {
                None => {
                    if lo < 0 {
                        let l = string.chars().count();
                        lo = max(0, l as i64 + lo);

                        chars.skip(lo as usize).collect()
                    } else {
                        chars.skip(lo as usize).collect()
                    }
                }
                Some(hi_value) => {
                    let mut hi = hi_value.try_as_i64()?;

                    if lo < 0 {
                        "".to_string()
                    } else {
                        if hi < 0 {
                            let l = string.chars().count();
                            hi = max(0, l as i64 + hi);
                        }

                        chars.skip(lo as usize).take((hi - lo) as usize).collect()
                    }
                }
            };

            Ok(DynamicValue::from(substring))
        }
        DynamicValue::List(_) => Err(EvaluationError::NotImplemented("list".to_string())),
        value => {
            return Err(EvaluationError::Cast((
                value.type_of().to_string(),
                "sequence".to_string(),
            )))
        }
    }
}

fn join(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    let list = arg1.try_as_list()?;
    let joiner = arg2.try_as_str()?;

    let mut string_list: Vec<Cow<str>> = Vec::new();

    for value in list {
        string_list.push(value.try_as_str()?);
    }

    Ok(DynamicValue::from(string_list.join(&joiner)))
}

fn contains(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    match arg1.as_ref() {
        DynamicValue::String(text) => match arg2.as_ref() {
            DynamicValue::Regex(pattern) => Ok(DynamicValue::from(pattern.is_match(text))),
            _ => {
                let pattern = arg2.try_as_str()?;
                Ok(DynamicValue::from(text.contains(&*pattern)))
            }
        },
        DynamicValue::List(_) => Err(EvaluationError::NotImplemented("list".to_string())),
        value => {
            return Err(EvaluationError::Cast((
                value.type_of().to_string(),
                "sequence".to_string(),
            )))
        }
    }
}

fn replace(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2, arg3) = args.get3();

    let string = arg1.try_as_str()?;
    let replacement = arg3.try_as_str()?;

    let replaced = match arg2.try_as_regex() {
        Ok(regex) => regex.replace_all(&string, replacement).into_owned(),
        Err(_) => {
            let pattern = arg2.try_as_str()?;

            string.replace(&*pattern, &replacement)
        }
    };

    Ok(DynamicValue::from(replaced))
}

fn compact(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1_list()?;

    Ok(DynamicValue::List(match arg {
        Cow::Borrowed(list) => list
            .iter()
            .filter(|value| value.is_truthy())
            .cloned()
            .collect(),
        Cow::Owned(mut list) => {
            list.retain(|value| value.is_truthy());
            list
        }
    }))
}

// Arithmetics
fn arithmetic_op<F>(args: BoundArguments, op: F) -> FunctionResult
where
    F: FnOnce(DynamicNumber, DynamicNumber) -> DynamicNumber,
{
    let (a, b) = args.get2_number()?;
    Ok(DynamicValue::from(op(a, b)))
}

fn variadic_arithmetic_op<F>(args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber, DynamicNumber) -> DynamicNumber,
{
    let mut args_iter = args.into_iter();

    let mut acc = args_iter.next().unwrap().try_as_number()?;

    for arg in args_iter {
        let cur = arg.try_as_number()?;
        acc = op(acc, cur);
    }

    Ok(DynamicValue::from(acc))
}

fn unary_arithmetic_op<F>(mut args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber) -> DynamicNumber,
{
    Ok(DynamicValue::from(op(args.pop1_number()?)))
}

fn binary_arithmetic_op<F>(args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber, DynamicNumber) -> DynamicNumber,
{
    let (n1, n2) = args.get2_number()?;

    Ok(DynamicValue::from(op(n1, n2)))
}

fn variadic_min(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let mut min_value = args_iter.next().unwrap().try_as_number()?;

    for arg in args_iter {
        let other_value = arg.try_as_number()?;

        if other_value < min_value {
            min_value = other_value;
        }
    }

    Ok(DynamicValue::from(min_value))
}

fn variadic_max(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let mut max_value = args_iter.next().unwrap().try_as_number()?;

    for arg in args_iter {
        let other_value = arg.try_as_number()?;

        if other_value > max_value {
            max_value = other_value;
        }
    }

    Ok(DynamicValue::from(max_value))
}

fn argmin(args: BoundArguments) -> FunctionResult {
    let values = args.get(0).unwrap().try_as_list()?;
    let mut min_item: Option<(DynamicNumber, DynamicValue)> = None;

    for (i, value) in values.iter().enumerate() {
        let n = value.try_as_number()?;

        match min_item {
            None => {
                min_item = Some((n, DynamicValue::from(i)));
            }
            Some((current, _)) => {
                if n < current {
                    min_item = Some((n, DynamicValue::from(i)));
                }
            }
        }
    }

    Ok(DynamicValue::from(min_item.map(|t| t.1)))
}

// Utilities
fn coalesce(args: BoundArguments) -> FunctionResult {
    for arg in args {
        if arg.is_truthy() {
            return Ok(arg.into_owned());
        }
    }

    Ok(DynamicValue::None)
}

// Boolean
fn not(mut args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(!args.pop1_bool()))
}

fn and(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(args.into_iter().all(|v| v.is_truthy())))
}

fn or(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(args.into_iter().any(|v| v.is_truthy())))
}

// Comparison
fn number_compare<F>(args: BoundArguments, validate: F) -> FunctionResult
where
    F: FnOnce(Ordering) -> bool,
{
    let (a, b) = args.get2_number()?;

    Ok(DynamicValue::from(match a.partial_cmp(&b) {
        Some(ordering) => validate(ordering),
        None => false,
    }))
}

fn sequence_compare<F>(args: BoundArguments, validate: F) -> FunctionResult
where
    F: FnOnce(Ordering) -> bool,
{
    // TODO: deal with lists
    let (a, b) = args.get2_str()?;

    Ok(DynamicValue::from(match a.partial_cmp(&b) {
        Some(ordering) => validate(ordering),
        None => false,
    }))
}

// IO
fn abspath(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;
    let mut path = PathBuf::new();
    path.push(arg.as_ref());
    let path = path.canonicalize().unwrap();
    let path = String::from(path.to_str().ok_or(EvaluationError::InvalidPath)?);

    Ok(DynamicValue::from(path))
}

fn pathjoin(args: BoundArguments) -> FunctionResult {
    let mut path = PathBuf::new();

    for arg in args {
        path.push(arg.try_as_str()?.as_ref());
    }

    let path = String::from(path.to_str().ok_or(EvaluationError::InvalidPath)?);

    Ok(DynamicValue::from(path))
}

fn decoder_trap_from_str(name: &str) -> Result<DecoderTrap, EvaluationError> {
    Ok(match name {
        "strict" => DecoderTrap::Strict,
        "replace" => DecoderTrap::Replace,
        "ignore" => DecoderTrap::Ignore,
        _ => return Err(EvaluationError::UnsupportedDecoderTrap(name.to_string())),
    })
}

fn isfile(args: BoundArguments) -> FunctionResult {
    let path = args.get1_str()?;
    let path = Path::new(path.as_ref());

    Ok(DynamicValue::Boolean(path.is_file()))
}

fn read(args: BoundArguments) -> FunctionResult {
    let path = args.get(0).unwrap().try_as_str()?;

    // TODO: handle encoding
    let mut file = match File::open(path.as_ref()) {
        Err(_) => return Err(EvaluationError::CannotOpenFile(path.into_owned())),
        Ok(f) => f,
    };

    let contents = match args.get(1) {
        Some(encoding_value) => {
            let encoding_name = encoding_value.try_as_str()?.replace('_', "-");
            let encoding = encoding_from_whatwg_label(&encoding_name);
            let encoding = encoding
                .ok_or_else(|| EvaluationError::UnsupportedEncoding(encoding_name.to_string()))?;

            let decoder_trap = match args.get(2) {
                Some(trap) => decoder_trap_from_str(&trap.try_as_str()?)?,
                None => DecoderTrap::Replace,
            };

            let mut buffer: Vec<u8> = Vec::new();

            if path.ends_with(".gz") {
                let mut gz = GzDecoder::new(file);
                gz.read_to_end(&mut buffer)
                    .map_err(|_| EvaluationError::CannotReadFile(path.into_owned()))?;
            } else {
                file.read_to_end(&mut buffer)
                    .map_err(|_| EvaluationError::CannotReadFile(path.into_owned()))?;
            }

            encoding
                .decode(&buffer, decoder_trap)
                .map_err(|_| EvaluationError::DecodeError)?
        }
        None => {
            let mut buffer = String::new();

            if path.ends_with(".gz") {
                let mut gz = GzDecoder::new(file);
                gz.read_to_string(&mut buffer)
                    .map_err(|_| EvaluationError::CannotReadFile(path.into_owned()))?;
            } else {
                file.read_to_string(&mut buffer)
                    .map_err(|_| EvaluationError::CannotReadFile(path.into_owned()))?;
            }

            buffer
        }
    };

    Ok(DynamicValue::from(contents))
}

fn write(args: BoundArguments) -> FunctionResult {
    let data = args.get(0).unwrap().try_as_str()?;
    let path = PathBuf::from(args.get(1).unwrap().try_as_str()?.as_ref());

    // mkdir -p
    if let Some(dir) = path.parent() {
        // NOTE: fs::create_dir_all is threadsafe
        fs::create_dir_all(dir)
            .map_err(|_| EvaluationError::CannotCreateDir(path.to_string_lossy().to_string()))?;
    }

    // TODO: this is not threadsafe
    fs::write(&path, data.as_bytes())
        .map_err(|_| EvaluationError::CannotWriteFile(path.to_string_lossy().to_string()))?;

    Ok(DynamicValue::from(path.to_string_lossy()))
}

fn ext(args: BoundArguments) -> FunctionResult {
    let path = PathBuf::from(args.get1_str()?.as_ref());

    Ok(DynamicValue::from(
        path.extension().and_then(|e| e.to_str()),
    ))
}

fn filesize(args: BoundArguments) -> FunctionResult {
    let path = args.get1_str()?;

    match fs::metadata(path.as_ref()) {
        Ok(size) => Ok(DynamicValue::from(size.len() as i64)),
        Err(_) => Err(EvaluationError::CannotOpenFile(path.into_owned())),
    }
}

fn bytesize(args: BoundArguments) -> FunctionResult {
    let bytes = args.get1().try_as_usize()? as u64;
    let human_readable = ByteSize::b(bytes).to_string();

    Ok(DynamicValue::from(human_readable))
}

// Introspection
fn type_of(mut args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(args.pop1().type_of()))
}

// Random
fn uuid(_args: BoundArguments) -> FunctionResult {
    let id = Uuid::new_v4()
        .to_hyphenated()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_string();

    Ok(DynamicValue::from(id))
}

// Utils
fn err(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;

    Err(EvaluationError::Custom(arg.to_string()))
}

fn val(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();
    Ok(arg.into_owned())
}

fn json_parse(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;

    serde_json::from_str(arg.as_ref()).map_err(|_| EvaluationError::JSONParseError)
}
