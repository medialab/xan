use std::borrow::Cow;
use std::cmp::max;
use std::cmp::{Ordering, PartialOrd};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use bytesize::ByteSize;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use flate2::read::GzDecoder;
use namedlock::{AutoCleanup, LockSpace};
use unidecode::unidecode;
use uuid::Uuid;

use super::agg::Welford;
use super::error::EvaluationError;
use super::types::{Argument, BoundArguments, DynamicNumber, DynamicValue, FunctionArguments};

type FunctionResult = Result<DynamicValue, EvaluationError>;
pub type Function = fn(BoundArguments) -> FunctionResult;

pub fn get_function(name: &str) -> Option<(Function, FunctionArguments)> {
    Some(match name {
        "__num_eq" => (
            |args| number_compare(args, Ordering::is_eq),
            FunctionArguments::binary(),
        ),
        "__num_gt" => (
            |args| number_compare(args, Ordering::is_gt),
            FunctionArguments::binary(),
        ),
        "__num_ge" => (
            |args| number_compare(args, Ordering::is_ge),
            FunctionArguments::binary(),
        ),
        "__num_lt" => (
            |args| number_compare(args, Ordering::is_lt),
            FunctionArguments::binary(),
        ),
        "__num_le" => (
            |args| number_compare(args, Ordering::is_le),
            FunctionArguments::binary(),
        ),
        "__num_ne" => (
            |args| number_compare(args, Ordering::is_ne),
            FunctionArguments::binary(),
        ),
        "abs" => (
            |args| unary_arithmetic_op(args, DynamicNumber::abs),
            FunctionArguments::unary(),
        ),
        "abspath" => (abspath, FunctionArguments::unary()),
        "add" => (
            |args| variadic_arithmetic_op(args, Add::add),
            FunctionArguments::variadic(2),
        ),
        "and" => (and, FunctionArguments::variadic(2)),
        "argmax" => (
            |args| argcompare(args, Ordering::is_gt),
            FunctionArguments::with_range(1..=2),
        ),
        "argmin" => (
            |args| argcompare(args, Ordering::is_lt),
            FunctionArguments::with_range(1..=2),
        ),
        "bytesize" => (bytesize, FunctionArguments::unary()),
        "ceil" => (
            |args| unary_arithmetic_op(args, DynamicNumber::ceil),
            FunctionArguments::unary(),
        ),
        "coalesce" => (coalesce, FunctionArguments::variadic(2)),
        "compact" => (compact, FunctionArguments::unary()),
        "concat" => (concat, FunctionArguments::variadic(2)),
        "contains" => (contains, FunctionArguments::binary()),
        "copy" => (copy_file, FunctionArguments::binary()),
        "count" => (count, FunctionArguments::binary()),
        "div" => (
            |args| variadic_arithmetic_op(args, Div::div),
            FunctionArguments::variadic(2),
        ),
        "endswith" => (endswith, FunctionArguments::binary()),
        "err" => (err, FunctionArguments::unary()),
        "escape_regex" => (escape_regex, FunctionArguments::unary()),
        "ext" => (ext, FunctionArguments::unary()),
        "filesize" => (filesize, FunctionArguments::unary()),
        "first" => (first, FunctionArguments::unary()),
        "floor" => (
            |args| unary_arithmetic_op(args, DynamicNumber::floor),
            FunctionArguments::unary(),
        ),
        "fmt" => (fmt, FunctionArguments::variadic(2)),
        "get" => (get, FunctionArguments::with_range(2..=3)),
        "idiv" => (
            |args| arithmetic_op(args, DynamicNumber::idiv),
            FunctionArguments::binary(),
        ),
        "isfile" => (isfile, FunctionArguments::unary()),
        "join" => (join, FunctionArguments::binary()),
        "json_parse" => (json_parse, FunctionArguments::unary()),
        "keys" => (keys, FunctionArguments::unary()),
        "last" => (last, FunctionArguments::unary()),
        "len" => (len, FunctionArguments::unary()),
        "log" => (
            |args| unary_arithmetic_op(args, DynamicNumber::ln),
            FunctionArguments::unary(),
        ),
        "ltrim" => (ltrim, FunctionArguments::with_range(1..=2)),
        "lower" => (lower, FunctionArguments::unary()),
        "match" => (regex_match, FunctionArguments::with_range(2..=3)),
        "max" => (variadic_max, FunctionArguments::variadic(2)),
        "md5" => (md5, FunctionArguments::unary()),
        "mean" => (mean, FunctionArguments::unary()),
        "min" => (variadic_min, FunctionArguments::variadic(2)),
        "mod" => (
            |args| binary_arithmetic_op(args, Rem::rem),
            FunctionArguments::binary(),
        ),
        "move" => (move_file, FunctionArguments::binary()),
        "mul" => (
            |args| variadic_arithmetic_op(args, Mul::mul),
            FunctionArguments::variadic(2),
        ),
        "neg" => (
            |args| unary_arithmetic_op(args, Neg::neg),
            FunctionArguments::unary(),
        ),
        "not" => (not, FunctionArguments::unary()),
        "or" => (or, FunctionArguments::variadic(2)),
        "pathjoin" => (pathjoin, FunctionArguments::variadic(2)),
        "pow" => (
            |args| binary_arithmetic_op(args, DynamicNumber::pow),
            FunctionArguments::binary(),
        ),
        "read" => (
            read,
            FunctionArguments::complex(vec![
                Argument::Positional,
                Argument::with_name("encoding"),
                Argument::with_name("errors"),
            ]),
        ),
        "read_csv" => (read_csv, FunctionArguments::unary()),
        "read_json" => (read_json, FunctionArguments::unary()),
        "replace" => (replace, FunctionArguments::nary(3)),
        "round" => (
            |args| unary_arithmetic_op(args, DynamicNumber::round),
            FunctionArguments::unary(),
        ),
        "rtrim" => (rtrim, FunctionArguments::with_range(1..=2)),
        "slice" => (slice, FunctionArguments::with_range(2..=3)),
        "split" => (split, FunctionArguments::with_range(2..=3)),
        "sqrt" => (
            |args| unary_arithmetic_op(args, DynamicNumber::sqrt),
            FunctionArguments::unary(),
        ),
        "startswith" => (startswith, FunctionArguments::binary()),
        "sub" => (
            |args| variadic_arithmetic_op(args, Sub::sub),
            FunctionArguments::variadic(2),
        ),
        "eq" => (
            |args| sequence_compare(args, Ordering::is_eq),
            FunctionArguments::binary(),
        ),
        "gt" => (
            |args| sequence_compare(args, Ordering::is_gt),
            FunctionArguments::binary(),
        ),
        "ge" => (
            |args| sequence_compare(args, Ordering::is_ge),
            FunctionArguments::binary(),
        ),
        "lt" => (
            |args| sequence_compare(args, Ordering::is_lt),
            FunctionArguments::binary(),
        ),
        "le" => (
            |args| sequence_compare(args, Ordering::is_le),
            FunctionArguments::binary(),
        ),
        "ne" => (
            |args| sequence_compare(args, Ordering::is_ne),
            FunctionArguments::binary(),
        ),
        "trim" => (trim, FunctionArguments::with_range(1..=2)),
        "trunc" => (
            |args| unary_arithmetic_op(args, DynamicNumber::trunc),
            FunctionArguments::unary(),
        ),
        "typeof" => (type_of, FunctionArguments::unary()),
        "unidecode" => (apply_unidecode, FunctionArguments::unary()),
        "upper" => (upper, FunctionArguments::unary()),
        "uuid" => (uuid, FunctionArguments::nullary()),
        "values" => (values, FunctionArguments::unary()),
        "write" => (write, FunctionArguments::binary()),
        _ => return None,
    })
}

// Strings
fn trim(args: BoundArguments) -> FunctionResult {
    let string = args.get(0).unwrap().try_as_str()?;
    let chars_opt = args.get(1);

    Ok(match chars_opt {
        None => DynamicValue::from(string.trim()),
        Some(chars) => {
            let pattern = chars.try_as_str()?.chars().collect::<Vec<char>>();
            DynamicValue::from(string.trim_matches(|c| pattern.contains(&c)))
        }
    })
}

fn ltrim(args: BoundArguments) -> FunctionResult {
    let string = args.get(0).unwrap().try_as_str()?;
    let chars_opt = args.get(1);

    Ok(match chars_opt {
        None => DynamicValue::from(string.trim_start()),
        Some(chars) => {
            let pattern = chars.try_as_str()?.chars().collect::<Vec<char>>();
            DynamicValue::from(string.trim_start_matches(|c| pattern.contains(&c)))
        }
    })
}

fn rtrim(args: BoundArguments) -> FunctionResult {
    let string = args.get(0).unwrap().try_as_str()?;
    let chars_opt = args.get(1);

    Ok(match chars_opt {
        None => DynamicValue::from(string.trim_end()),
        Some(chars) => {
            let pattern = chars.try_as_str()?.chars().collect::<Vec<char>>();
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
    let to_split = args.get(0).unwrap().try_as_str()?;
    let pattern = args.get(1).unwrap().try_as_str()?;
    let count = args.get(2);

    let splitted: Vec<DynamicValue> = if let Some(c) = count {
        to_split
            .splitn(c.try_as_usize()? + 1, pattern.as_ref())
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

    Ok(DynamicValue::from(match arg {
        DynamicValue::List(list) => list.len(),
        DynamicValue::Map(map) => map.len(),
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

    Ok(DynamicValue::from(string.starts_with(pattern.as_ref())))
}

fn endswith(args: BoundArguments) -> FunctionResult {
    let (string, pattern) = args.get2_str()?;

    Ok(DynamicValue::from(string.ends_with(pattern.as_ref())))
}

fn concat(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let first = args_iter.next().unwrap();

    match first {
        // NOTE: if the list's arc has a single reference, we can safely
        // mutate it because it belongs to the pipeline
        DynamicValue::List(list) => match Arc::try_unwrap(list) {
            Ok(mut owned_list) => {
                for arg in args_iter {
                    owned_list.push(arg);
                }

                Ok(DynamicValue::from(owned_list))
            }
            Err(borrowed_list) => {
                let mut result = Vec::clone(&borrowed_list);

                for arg in args_iter {
                    result.push(arg);
                }

                Ok(DynamicValue::from(result))
            }
        },
        value => {
            let first_part = value.try_as_str()?;

            let mut result = String::with_capacity(first_part.len());
            result.push_str(&first_part);

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
        DynamicValue::String(string) => DynamicValue::from(string.chars().next()),
        DynamicValue::List(list) => match list.first() {
            None => DynamicValue::None,
            Some(value) => value.clone(),
        },
        _ => return Err(EvaluationError::from_cast(&arg, "sequence")),
    })
}

fn last(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(match arg {
        DynamicValue::String(string) => DynamicValue::from(string.chars().next_back()),
        DynamicValue::List(list) => match list.last() {
            None => DynamicValue::None,
            Some(value) => value.clone(),
        },
        _ => return Err(EvaluationError::from_cast(&arg, "sequence")),
    })
}

fn get_subroutine<'a>(
    target: &'a DynamicValue,
    key: &'a DynamicValue,
) -> Result<Option<DynamicValue>, EvaluationError> {
    Ok(match target {
        DynamicValue::String(value) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += value.len() as i64;
            }

            if index < 0 {
                None
            } else {
                value.chars().nth(index as usize).map(DynamicValue::from)
            }
        }
        DynamicValue::List(list) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += list.len() as i64;
            }

            if index < 0 {
                None
            } else {
                list.get(index as usize).cloned()
            }
        }
        DynamicValue::Map(map) => {
            let key = key.try_as_str()?;

            map.get(key.as_ref()).cloned()
        }
        value => return Err(EvaluationError::from_cast(value, "sequence")),
    })
}

fn get(mut args: BoundArguments) -> FunctionResult {
    let (target, key, default) = if args.len() == 3 {
        let (target, key, default) = args.pop3();

        (target, key, Some(default))
    } else {
        let (target, key) = args.pop2();

        (target, key, None)
    };

    match key {
        DynamicValue::List(path) => {
            let mut current = target;

            for step in path.iter() {
                match get_subroutine(&current, step)? {
                    None => return Ok(default.unwrap_or_else(|| DynamicValue::None)),
                    Some(next) => current = next,
                }
            }

            Ok(current)
        }
        _ => Ok(get_subroutine(&target, &key)?
            .unwrap_or_else(|| default.unwrap_or_else(|| DynamicValue::None))),
    }
}

fn slice(args: BoundArguments) -> FunctionResult {
    let target = args.get(0).unwrap();

    match target {
        DynamicValue::String(string) => {
            let mut lo = args.get(1).unwrap().try_as_i64()?;
            let opt_hi = args.get(2);

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
        DynamicValue::List(list) => {
            // TODO: can be implemented through Arc::try_unwrap
            let mut lo = args.get(1).unwrap().try_as_i64()?;
            let opt_hi = args.get(2);

            let sublist: Vec<DynamicValue> = match opt_hi {
                None => {
                    if lo < 0 {
                        let l = list.len();
                        lo = max(0, l as i64 + lo);

                        list[..lo as usize].to_vec()
                    } else {
                        list[..lo as usize].to_vec()
                    }
                }
                Some(hi_value) => {
                    let mut hi = hi_value.try_as_i64()?;

                    if lo < 0 {
                        Vec::new()
                    } else {
                        if hi < 0 {
                            let l = list.len();
                            hi = max(0, l as i64 + hi);
                        }

                        list[lo as usize..(hi - lo) as usize].to_vec()
                    }
                }
            };

            Ok(DynamicValue::from(sublist))
        }
        value => Err(EvaluationError::from_cast(value, "sequence")),
    }
}

fn join(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    let list = arg1.try_as_list()?;
    let joiner = arg2.try_as_str()?;

    let mut string_list: Vec<Cow<str>> = Vec::with_capacity(list.len());

    for value in list.iter() {
        string_list.push(value.try_as_str()?);
    }

    Ok(DynamicValue::from(string_list.join(&joiner)))
}

fn contains(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();

    match arg1 {
        DynamicValue::String(text) => match arg2 {
            DynamicValue::Regex(pattern) => Ok(DynamicValue::from(pattern.is_match(text))),
            _ => {
                let pattern = arg2.try_as_str()?;
                Ok(DynamicValue::from(text.contains(pattern.as_ref())))
            }
        },
        DynamicValue::List(list) => {
            let needle = arg2.try_as_str()?;

            for item in list.iter() {
                if needle == item.try_as_str()? {
                    return Ok(DynamicValue::from(true));
                }
            }

            Ok(DynamicValue::from(false))
        }
        DynamicValue::Map(map) => {
            let needle = arg2.try_as_str()?;

            return Ok(DynamicValue::from(map.contains_key(needle.as_ref())));
        }
        value => Err(EvaluationError::from_cast(value, "sequence")),
    }
}

fn regex_match(args: BoundArguments) -> FunctionResult {
    let haystack = args.get(0).unwrap().try_as_str()?;
    let pattern = args.get(1).unwrap().try_as_regex()?;
    let group = args
        .get(2)
        .map(|v| v.try_as_usize())
        .transpose()?
        .unwrap_or(0);

    if let Some(caps) = pattern.captures(haystack.as_ref()) {
        Ok(DynamicValue::from(caps.get(group).map(|g| g.as_str())))
    } else {
        Ok(DynamicValue::None)
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
    let arg = args.pop1();
    let list = arg.try_into_arc_list()?;

    Ok(match Arc::try_unwrap(list) {
        Err(borrowed_list) => DynamicValue::from(
            borrowed_list
                .iter()
                .filter(|value| value.is_truthy())
                .cloned()
                .collect::<Vec<_>>(),
        ),
        Ok(mut owned_list) => {
            owned_list.retain(|v| v.is_truthy());
            DynamicValue::from(owned_list)
        }
    })
}

// Maps
fn keys(args: BoundArguments) -> FunctionResult {
    let map = args.get1().try_as_map()?;

    Ok(DynamicValue::from(
        map.keys()
            .map(|k| DynamicValue::from(k.as_str()))
            .collect::<Vec<_>>(),
    ))
}

fn values(args: BoundArguments) -> FunctionResult {
    let map = args.get1().try_as_map()?;

    Ok(DynamicValue::from(
        map.values().cloned().collect::<Vec<_>>(),
    ))
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
    if args.len() == 1 {
        let values = args.get1().try_as_list()?;

        if values.is_empty() {
            return Ok(DynamicValue::None);
        }

        let mut values_iter = values.iter();
        let mut min_value = values_iter.next().unwrap().try_as_number()?;

        for value in values_iter {
            let other = value.try_as_number()?;

            if other < min_value {
                min_value = other;
            }
        }

        return Ok(DynamicValue::from(min_value));
    }

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
    if args.len() == 1 {
        let values = args.get1().try_as_list()?;

        if values.is_empty() {
            return Ok(DynamicValue::None);
        }

        let mut values_iter = values.iter();
        let mut max_value = values_iter.next().unwrap().try_as_number()?;

        for value in values_iter {
            let other = value.try_as_number()?;

            if other > max_value {
                max_value = other;
            }
        }

        return Ok(DynamicValue::from(max_value));
    }

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

fn argcompare<F>(args: BoundArguments, validate: F) -> FunctionResult
where
    F: Fn(Ordering) -> bool,
{
    let values = args.get(0).unwrap().try_as_list()?;
    let labels = args.get(1).map(|arg| arg.try_as_list()).transpose()?;
    let mut min_item: Option<(DynamicNumber, DynamicValue)> = None;

    for (i, value) in values.iter().enumerate() {
        let n = value.try_as_number()?;

        match min_item {
            None => {
                min_item = Some((
                    n,
                    match labels {
                        None => DynamicValue::from(i),
                        Some(l) => l.get(i).cloned().unwrap_or_else(|| DynamicValue::None),
                    },
                ));
            }
            Some((current, _)) => {
                if validate(n.partial_cmp(&current).unwrap()) {
                    min_item = Some((
                        n,
                        match labels {
                            None => DynamicValue::from(i),
                            Some(l) => l.get(i).cloned().unwrap_or_else(|| DynamicValue::None),
                        },
                    ));
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
            return Ok(arg);
        }
    }

    Ok(DynamicValue::None)
}

// Boolean
fn not(mut args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(!args.pop1_bool()))
}

fn and(args: BoundArguments) -> FunctionResult {
    let mut last: Option<DynamicValue> = None;

    for arg in args {
        if arg.is_falsey() {
            return Ok(arg);
        }

        last = Some(arg);
    }

    Ok(last.unwrap())
}

fn or(args: BoundArguments) -> FunctionResult {
    let mut last: Option<DynamicValue> = None;

    for arg in args {
        if arg.is_truthy() {
            return Ok(arg);
        }

        last = Some(arg);
    }

    Ok(last.unwrap())
}

// TODO: rewrap those to take lists instead, since the variadic usage is mostly moot
// fn all(args: BoundArguments) -> FunctionResult {
//     Ok(DynamicValue::from(args.into_iter().all(|v| v.is_truthy())))
// }

// fn any(args: BoundArguments) -> FunctionResult {
//     Ok(DynamicValue::from(args.into_iter().any(|v| v.is_truthy())))
// }

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

// Aggregation
fn mean(args: BoundArguments) -> FunctionResult {
    let items = args.get1().try_as_list()?;
    let mut welford = Welford::new();

    for item in items {
        let n = item.try_as_f64()?;
        welford.add(n);
    }

    Ok(DynamicValue::from(welford.mean()))
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

fn abstract_read(
    path: &DynamicValue,
    encoding: Option<&DynamicValue>,
    errors: Option<&DynamicValue>,
) -> Result<String, EvaluationError> {
    let path = path.try_as_str()?;

    let mut file = match File::open(path.as_ref()) {
        Err(_) => return Err(EvaluationError::IO(format!("cannot read file {}", path))),
        Ok(f) => f,
    };

    let contents = match encoding {
        Some(encoding_value) => {
            let encoding_name = encoding_value.try_as_str()?.replace('_', "-");
            let encoding = encoding_from_whatwg_label(&encoding_name);
            let encoding = encoding
                .ok_or_else(|| EvaluationError::UnsupportedEncoding(encoding_name.to_string()))?;

            let decoder_trap = match errors {
                Some(trap) => decoder_trap_from_str(&trap.try_as_str()?)?,
                None => DecoderTrap::Replace,
            };

            let mut buffer: Vec<u8> = Vec::new();

            if path.ends_with(".gz") {
                let mut gz = GzDecoder::new(file);
                gz.read_to_end(&mut buffer)
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
            } else {
                file.read_to_end(&mut buffer)
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
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
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
            } else {
                file.read_to_string(&mut buffer)
                    .map_err(|_| EvaluationError::IO(format!("cannot read file {}", path)))?;
            }

            buffer
        }
    };

    Ok(contents)
}

fn read(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(abstract_read(
        args.get1(),
        args.get_not_none(1),
        args.get_not_none(2),
    )?))
}

fn read_json(args: BoundArguments) -> FunctionResult {
    let contents = abstract_read(args.get1(), None, None)?;
    serde_json::from_str(&contents).map_err(|_| EvaluationError::JSONParseError)
}

fn read_csv(args: BoundArguments) -> FunctionResult {
    let contents = abstract_read(args.get1(), None, None)?;

    let mut reader = csv::Reader::from_reader(contents.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| EvaluationError::IO("error while reading CSV header row".to_string()))?
        .clone();

    let mut record = csv::StringRecord::new();
    let mut rows: Vec<DynamicValue> = Vec::new();

    loop {
        match reader.read_record(&mut record) {
            Err(_) => {
                return Err(EvaluationError::IO(
                    "error while reading CSV row".to_string(),
                ))
            }
            Ok(has_row) => {
                if !has_row {
                    break;
                }

                let mut map: HashMap<String, DynamicValue> = HashMap::with_capacity(headers.len());

                for (cell, header) in record.iter().zip(headers.iter()) {
                    map.insert(header.to_string(), DynamicValue::from(cell));
                }

                rows.push(DynamicValue::from(map));
            }
        }
    }

    Ok(DynamicValue::from(rows))
}

lazy_static! {
    static ref WRITE_FILE_LOCKS: LockSpace<PathBuf, ()> = LockSpace::new(AutoCleanup);
}

fn write(args: BoundArguments) -> FunctionResult {
    let data = args.get(0).unwrap().try_as_str()?;
    let path = PathBuf::from(args.get(1).unwrap().try_as_str()?.as_ref());

    // mkdir -p
    if let Some(dir) = path.parent() {
        // NOTE: fs::create_dir_all is threadsafe
        fs::create_dir_all(dir).map_err(|_| {
            EvaluationError::IO(format!("cannot create dir {}", dir.to_string_lossy()))
        })?;
    }

    WRITE_FILE_LOCKS
        .lock(path.clone(), || ())
        .map_err(|_| EvaluationError::Custom("write file lock is poisoned".to_string()))?;

    fs::write(&path, data.as_bytes()).map_err(|_| {
        EvaluationError::IO(format!("cannot write file {}", path.to_string_lossy()))
    })?;

    Ok(DynamicValue::from(path.to_string_lossy()))
}

fn move_file(args: BoundArguments) -> FunctionResult {
    let (source, target) = args.get2_str()?;

    let source_path = PathBuf::from(source.as_ref());
    let target_path = PathBuf::from(target.as_ref());

    // mkdir -p
    if let Some(dir) = target_path.parent() {
        // NOTE: fs::create_dir_all is threadsafe
        fs::create_dir_all(dir).map_err(|_| {
            EvaluationError::IO(format!("cannot create dir {}", dir.to_string_lossy()))
        })?;
    }

    fs::rename(&source_path, &target_path).map_err(|_| {
        EvaluationError::IO(format!(
            "cannot move from {} to {}",
            source_path.to_string_lossy(),
            target_path.to_string_lossy()
        ))
    })?;

    Ok(DynamicValue::from(target_path.to_string_lossy()))
}

fn copy_file(args: BoundArguments) -> FunctionResult {
    let (source, target) = args.get2_str()?;

    let source_path = PathBuf::from(source.as_ref());
    let target_path = PathBuf::from(target.as_ref());

    // mkdir -p
    if let Some(dir) = target_path.parent() {
        // NOTE: fs::create_dir_all is threadsafe
        fs::create_dir_all(dir).map_err(|_| {
            EvaluationError::IO(format!("cannot create dir {}", dir.to_string_lossy()))
        })?;
    }

    fs::copy(&source_path, &target_path).map_err(|_| {
        EvaluationError::IO(format!(
            "cannot copy {} to {}",
            source_path.to_string_lossy(),
            target_path.to_string_lossy()
        ))
    })?;

    Ok(DynamicValue::from(target_path.to_string_lossy()))
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
        Err(_) => Err(EvaluationError::IO(format!(
            "cannot access file metadata for {}",
            path
        ))),
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

fn json_parse(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;

    serde_json::from_str(arg.as_ref()).map_err(|_| EvaluationError::JSONParseError)
}
