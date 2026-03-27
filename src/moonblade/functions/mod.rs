use std::borrow::Cow;
use std::cmp::{max, Ordering, PartialOrd};
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};
use std::sync::Arc;

use base64::prelude::*;
use bstr::ByteSlice;
use lazy_static::lazy_static;
use mime2ext::mime2ext;
use paltoquet::{
    phonetics::{phonogram, refined_soundex, soundex},
    stemmers::{fr::carry_stemmer, s_stemmer},
    tokenizers::FingerprintTokenizer,
};
use rand::Rng;
use regex::Regex;
use unidecode::unidecode;
use uuid::Uuid;

use crate::collections::HashMap;

use super::agg::aggregators::{Sum, Welford};
use super::error::EvaluationError;
use super::types::{Argument, BoundArguments, DynamicNumber, DynamicValue, FunctionArguments};

mod fmt;
mod io;
pub mod special;
mod time;
mod urls;

pub type FunctionResult = Result<DynamicValue, EvaluationError>;
pub type Function = fn(BoundArguments) -> FunctionResult;

pub fn get_function(name: &str) -> Option<(Function, FunctionArguments)> {
    Some(match name {
        // Operators
        "==" => (
            |args| abstract_compare(args, Ordering::is_eq),
            FunctionArguments::binary(),
        ),
        ">" => (
            |args| abstract_compare(args, Ordering::is_gt),
            FunctionArguments::binary(),
        ),
        ">=" => (
            |args| abstract_compare(args, Ordering::is_ge),
            FunctionArguments::binary(),
        ),
        "<" => (
            |args| abstract_compare(args, Ordering::is_lt),
            FunctionArguments::binary(),
        ),
        "<=" => (
            |args| abstract_compare(args, Ordering::is_le),
            FunctionArguments::binary(),
        ),
        "!=" => (
            |args| abstract_compare(args, Ordering::is_ne),
            FunctionArguments::binary(),
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

        // Functions
        "abs" => (
            |args| unary_arithmetic_op(args, DynamicNumber::abs),
            FunctionArguments::unary(),
        ),
        "abspath" => (io::abspath, FunctionArguments::unary()),
        "add" => (
            |args| variadic_arithmetic_op(args, Add::add),
            FunctionArguments::variadic(2),
        ),
        "argmax" => (
            |args| argcompare(args, Ordering::is_gt),
            FunctionArguments::with_range(1..=2),
        ),
        "argmin" => (
            |args| argcompare(args, Ordering::is_lt),
            FunctionArguments::with_range(1..=2),
        ),
        "basename" => (io::basename, FunctionArguments::with_range(1..=2)),
        "bytesize" => (io::bytesize, FunctionArguments::unary()),
        "carry_stemmer" => (
            |args| abstract_unary_string_fn(args, |string| Cow::Owned(carry_stemmer(string))),
            FunctionArguments::unary(),
        ),
        "ceil" => (
            |args| round_like_op(args, DynamicNumber::ceil),
            FunctionArguments::with_range(1..=2),
        ),
        "cmd" => (io::cmd, FunctionArguments::binary()),
        "compact" => (compact, FunctionArguments::unary()),
        "concat" => (concat, FunctionArguments::variadic(2)),
        "contains" => (contains, FunctionArguments::binary()),
        "copy" => (io::copy_file, FunctionArguments::binary()),
        "count" => (count, FunctionArguments::binary()),
        "date" => (time::date, FunctionArguments::with_range(1..=2)),
        "datetime" => (time::datetime, FunctionArguments::with_range(1..=2)),
        "dirname" => (io::dirname, FunctionArguments::unary()),
        "div" => (
            |args| variadic_arithmetic_op(args, Div::div),
            FunctionArguments::variadic(2),
        ),
        // "earliest" => (
        //     |args| {
        //         variadic_optimum(
        //             args,
        //             |value| value.try_as_datetime().map(Cow::into_owned),
        //             Ordering::is_lt,
        //         )
        //     },
        //     FunctionArguments::variadic(1),
        // ),
        "endswith" => (endswith, FunctionArguments::binary()),
        "err" => (err, FunctionArguments::unary()),
        "escape_regex" => (escape_regex, FunctionArguments::unary()),
        "ext" => (io::ext, FunctionArguments::unary()),
        "filesize" => (io::filesize, FunctionArguments::unary()),
        "fingerprint" => (fingerprint, FunctionArguments::unary()),
        "first" => (first, FunctionArguments::unary()),
        "float" => (parse_float, FunctionArguments::unary()),
        "floor" => (
            |args| round_like_op(args, DynamicNumber::floor),
            FunctionArguments::with_range(1..=2),
        ),
        "fmt" => (fmt::fmt, FunctionArguments::variadic(2)),
        "from_timestamp" => (time::from_timestamp, FunctionArguments::unary()),
        "from_timestamp_ms" => (time::from_timestamp_ms, FunctionArguments::unary()),
        "numfmt" => (
            fmt::fmt_number,
            FunctionArguments::complex(vec![
                Argument::Positional,
                Argument::with_name("thousands_sep"),
                Argument::with_name("comma"),
                Argument::with_name("significance"),
            ]),
        ),
        "get" => (get, FunctionArguments::with_range(2..=3)),
        "html_unescape" => (html_unescape, FunctionArguments::unary()),
        "idiv" => (
            |args| arithmetic_op(args, DynamicNumber::idiv),
            FunctionArguments::binary(),
        ),
        "index_by" => (index_by, FunctionArguments::binary()),
        "int" => (parse_int, FunctionArguments::unary()),
        "isfile" => (io::isfile, FunctionArguments::unary()),
        "join" => (join, FunctionArguments::binary()),
        "keys" => (keys, FunctionArguments::unary()),
        // "latest" => (
        //     |args| {
        //         variadic_optimum(
        //             args,
        //             |value| value.try_as_datetime().map(Cow::into_owned),
        //             Ordering::is_gt,
        //         )
        //     },
        //     FunctionArguments::variadic(1),
        // ),
        "last" => (last, FunctionArguments::unary()),
        "len" => (len, FunctionArguments::unary()),
        "log" => (
            |args| match args.len() {
                1 => unary_arithmetic_op(args, DynamicNumber::ln),
                2 => binary_arithmetic_op(args, DynamicNumber::log),
                _ => unreachable!(),
            },
            FunctionArguments::with_range(1..=2),
        ),
        "log2" => (
            |args| unary_arithmetic_op(args, DynamicNumber::log2),
            FunctionArguments::unary(),
        ),
        "log10" => (
            |args| unary_arithmetic_op(args, DynamicNumber::log10),
            FunctionArguments::unary(),
        ),
        "lower" => (lower, FunctionArguments::unary()),
        "lru" => (urls::lru, FunctionArguments::unary()),
        "match" => (regex_match, FunctionArguments::with_range(2..=3)),
        "max" => (
            |args| variadic_optimum(args, DynamicValue::try_as_number, Ordering::is_gt),
            FunctionArguments::variadic(1),
        ),
        "md5" => (md5, FunctionArguments::unary()),
        "mean" => (mean, FunctionArguments::unary()),
        "mime_ext" => (mime_ext, FunctionArguments::unary()),
        "min" => (
            |args| variadic_optimum(args, DynamicValue::try_as_number, Ordering::is_lt),
            FunctionArguments::variadic(1),
        ),
        "mod" => (
            |args| binary_arithmetic_op(args, Rem::rem),
            FunctionArguments::binary(),
        ),
        "month" => (
            |args| time::custom_strftime(args, "%m"),
            FunctionArguments::unary(),
        ),
        "month_day" => (
            |args| time::custom_strftime(args, "%m-%d"),
            FunctionArguments::unary(),
        ),
        "move" => (io::move_file, FunctionArguments::binary()),
        "mul" => (
            |args| variadic_arithmetic_op(args, Mul::mul),
            FunctionArguments::variadic(2),
        ),
        "neg" => (
            |args| unary_arithmetic_op(args, Neg::neg),
            FunctionArguments::unary(),
        ),
        "not" => (not, FunctionArguments::unary()),
        "now" => (time::now, FunctionArguments::nullary()),
        "pad" => (
            |args| fmt::pad(pad::Alignment::Middle, args),
            FunctionArguments::with_range(2..=3),
        ),
        "lpad" => (
            |args| fmt::pad(pad::Alignment::Right, args),
            FunctionArguments::with_range(2..=3),
        ),
        "rpad" => (
            |args| fmt::pad(pad::Alignment::Left, args),
            FunctionArguments::with_range(2..=3),
        ),
        "parse_dataurl" => (parse_dataurl, FunctionArguments::unary()),
        "parse_json" => (parse_json, FunctionArguments::unary()),
        "parse_py_literal" => (parse_py_literal, FunctionArguments::unary()),
        "phonogram" => (
            |args| abstract_unary_string_fn(args, |string| Cow::Owned(phonogram(string))),
            FunctionArguments::unary(),
        ),
        "pjoin" | "pathjoin" => (io::pathjoin, FunctionArguments::variadic(2)),
        "pow" => (
            |args| binary_arithmetic_op(args, DynamicNumber::pow),
            FunctionArguments::binary(),
        ),
        "printf" => (fmt::printf, FunctionArguments::variadic(2)),
        "random" => (random, FunctionArguments::nullary()),
        "range" => (range, FunctionArguments::with_range(1..=3)),
        "read" => (
            io::read,
            FunctionArguments::complex(vec![
                Argument::Positional,
                Argument::with_name("encoding"),
                Argument::with_name("errors"),
            ]),
        ),
        "read_csv" => (io::read_csv, FunctionArguments::unary()),
        "read_json" => (io::read_json, FunctionArguments::unary()),
        "refined_soundex" => (
            |args| abstract_unary_string_fn(args, |string| Cow::Owned(refined_soundex(string))),
            FunctionArguments::unary(),
        ),
        "regex" => (parse_regex, FunctionArguments::unary()),
        "repeat" => (repeat, FunctionArguments::binary()),
        "replace" => (replace, FunctionArguments::nary(3)),
        "round" => (
            |args| round_like_op(args, DynamicNumber::round),
            FunctionArguments::with_range(1..=2),
        ),
        "shell" => (io::shell, FunctionArguments::unary()),
        "shlex_split" => (io::shlex_split, FunctionArguments::unary()),
        "slice" => (slice, FunctionArguments::with_range(2..=3)),
        "soundex" => (
            |args| abstract_unary_string_fn(args, |string| Cow::Owned(soundex(string))),
            FunctionArguments::unary(),
        ),
        "split" => (split, FunctionArguments::with_range(2..=3)),
        "sqrt" => (
            |args| unary_arithmetic_op(args, DynamicNumber::sqrt),
            FunctionArguments::unary(),
        ),
        "startswith" => (startswith, FunctionArguments::binary()),
        "strftime" => (time::strftime, FunctionArguments::binary()),
        "sub" => (
            |args| variadic_arithmetic_op(args, Sub::sub),
            FunctionArguments::variadic(2),
        ),
        "sum" => (sum, FunctionArguments::unary()),
        "s_stemmer" => (
            |args| abstract_unary_string_fn(args, s_stemmer),
            FunctionArguments::unary(),
        ),
        "time" => (time::time, FunctionArguments::with_range(1..=2)),
        "to_fixed" => (fmt::to_fixed, FunctionArguments::binary()),
        "to_timestamp" => (time::to_timestamp, FunctionArguments::unary()),
        "to_timestamp_ms" => (time::to_timestamp_ms, FunctionArguments::unary()),
        "to_timezone" => (time::to_timezone, FunctionArguments::binary()),
        "to_local_timezone" => (time::to_local_timezone, FunctionArguments::unary()),
        "trim" => (fmt::trim, FunctionArguments::with_range(1..=2)),
        "ltrim" => (fmt::ltrim, FunctionArguments::with_range(1..=2)),
        "rtrim" => (fmt::rtrim, FunctionArguments::with_range(1..=2)),
        "trunc" => (
            |args| round_like_op(args, DynamicNumber::trunc),
            FunctionArguments::with_range(1..=2),
        ),
        "typeof" => (type_of, FunctionArguments::unary()),
        "unidecode" => (apply_unidecode, FunctionArguments::unary()),
        "upper" => (upper, FunctionArguments::unary()),
        "urljoin" => (urls::urljoin, FunctionArguments::binary()),
        "uuid" => (uuid, FunctionArguments::nullary()),
        "values" => (values, FunctionArguments::unary()),
        "with_timezone" => (time::with_timezone, FunctionArguments::binary()),
        "with_local_timezone" => (time::with_local_timezone, FunctionArguments::unary()),
        "without_timezone" => (time::without_timezone, FunctionArguments::unary()),
        "write" => (io::write, FunctionArguments::binary()),
        "year" => (
            |args| time::custom_strftime(args, "%Y"),
            FunctionArguments::unary(),
        ),
        "year_month_day" | "ymd" => (
            |args| time::custom_strftime(args, "%F"),
            FunctionArguments::unary(),
        ),
        "year_month" | "ym" => (
            |args| time::custom_strftime(args, "%Y-%m"),
            FunctionArguments::unary(),
        ),
        _ => return None,
    })
}

// Strings
fn escape_regex(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(regex::escape(args.get1_str()?.as_ref())))
}

fn md5(args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(format!(
        "{:x}",
        md5::compute(args.get1().try_as_bytes()?)
    )))
}

fn split(args: BoundArguments) -> FunctionResult {
    let to_split = args.get(0).unwrap().try_as_str()?;
    let pattern_arg = args.get(1).unwrap();
    let count = args.get(2);

    let splitted: Vec<DynamicValue> = if let DynamicValue::Regex(pattern) = pattern_arg {
        if let Some(c) = count {
            pattern
                .splitn(&to_split, c.try_as_usize()? + 1)
                .map(DynamicValue::from)
                .collect()
        } else {
            pattern.split(&to_split).map(DynamicValue::from).collect()
        }
    } else {
        let pattern = pattern_arg.try_as_str()?;

        if let Some(c) = count {
            to_split
                .splitn(c.try_as_usize()? + 1, pattern.as_ref())
                .map(DynamicValue::from)
                .collect()
        } else {
            to_split.split(&*pattern).map(DynamicValue::from).collect()
        }
    };

    Ok(DynamicValue::from(splitted))
}

fn lower(args: BoundArguments) -> FunctionResult {
    Ok(match args.get1() {
        DynamicValue::Bytes(bytes) => DynamicValue::from_owned_bytes(bytes.to_lowercase()),
        value => DynamicValue::from(value.try_as_str()?.to_lowercase()),
    })
}

fn upper(args: BoundArguments) -> FunctionResult {
    Ok(match args.get1() {
        DynamicValue::Bytes(bytes) => DynamicValue::from_owned_bytes(bytes.to_uppercase()),
        value => DynamicValue::from(value.try_as_str()?.to_uppercase()),
    })
}

fn len(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(DynamicValue::from(match arg {
        DynamicValue::List(list) => list.len(),
        DynamicValue::Map(map) => map.len(),
        _ => arg.try_as_str()?.chars().count(),
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

// Lists & Sequences
fn first(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    Ok(match arg {
        DynamicValue::String(string) => DynamicValue::from(string.chars().next()),
        DynamicValue::Bytes(bytes) => DynamicValue::from(
            std::str::from_utf8(&bytes)
                .map_err(|_| EvaluationError::UnicodeDecodeError)?
                .chars()
                .next(),
        ),
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
        DynamicValue::Bytes(bytes) => DynamicValue::from(
            std::str::from_utf8(&bytes)
                .map_err(|_| EvaluationError::UnicodeDecodeError)?
                .chars()
                .next_back(),
        ),
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
        DynamicValue::Bytes(value) => {
            let mut index = key.try_as_i64()?;

            if index < 0 {
                index += value.len() as i64;
            }

            if index < 0 {
                None
            } else {
                let value =
                    std::str::from_utf8(value).map_err(|_| EvaluationError::UnicodeDecodeError)?;

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

fn range(args: BoundArguments) -> FunctionResult {
    let (start, stop, step): (i64, i64, i64) = if args.len() == 3 {
        (
            args.get(0).unwrap().try_as_i64()?,
            args.get(1).unwrap().try_as_i64()?,
            args.get(2).unwrap().try_as_i64()?,
        )
    } else if args.len() == 2 {
        (
            args.get(0).unwrap().try_as_i64()?,
            args.get(1).unwrap().try_as_i64()?,
            1,
        )
    } else {
        (0, args.get(0).unwrap().try_as_i64()?, 1)
    };

    if step == 0 {
        return Err(EvaluationError::Custom("step cannot be 0".to_string()));
    }

    let len = if step > 0 {
        if start >= stop {
            0
        } else {
            ((stop - start - 1) / step + 1) as usize
        }
    } else if start <= stop {
        0
    } else {
        ((start - stop - 1) / (-step) + 1) as usize
    };

    let mut indices = Vec::with_capacity(len);

    let mut current = start;

    if step > 0 {
        while current < stop {
            indices.push(DynamicValue::from(current));
            current += step;
        }
    } else {
        while current > stop {
            indices.push(DynamicValue::from(current));
            current += step;
        }
    }

    Ok(DynamicValue::from(indices))
}

fn repeat(args: BoundArguments) -> FunctionResult {
    let (to_repeat_arg, times_arg) = args.get2();

    let times = times_arg.try_as_usize()?;

    if let DynamicValue::List(items) = to_repeat_arg {
        let mut repeated = Vec::with_capacity(items.len() * times);

        for _ in 0..times {
            for item in items.iter() {
                repeated.push(item.clone());
            }
        }

        Ok(DynamicValue::from(repeated))
    } else {
        let to_repeat = to_repeat_arg.try_as_str()?;

        let mut repeated = String::with_capacity(to_repeat.len() * times);

        for _ in 0..times {
            repeated.push_str(&to_repeat);
        }

        Ok(DynamicValue::from(repeated))
    }
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

    if let DynamicValue::List(list) = target {
        // TODO: can be implemented through Arc::try_unwrap
        let mut lo = args.get(1).unwrap().try_as_i64()?;
        let opt_hi = args.get(2);

        let sublist: Vec<DynamicValue> = match opt_hi {
            None => {
                if lo < 0 {
                    let l = list.len();
                    lo = max(0, l as i64 + lo);

                    list[..lo as usize].to_vec()
                } else if lo >= list.len() as i64 {
                    Vec::new()
                } else {
                    list[..lo as usize].to_vec()
                }
            }
            Some(hi_value) => {
                let mut hi = hi_value.try_as_i64()?;

                if lo >= list.len() as i64 {
                    Vec::new()
                } else if lo < 0 {
                    let l = list.len();

                    lo = max(0, l as i64 + lo);

                    if hi < 0 {
                        hi = max(0, l as i64 + hi);
                    }

                    if hi <= lo {
                        Vec::new()
                    } else {
                        list[lo as usize..hi.min(list.len() as i64) as usize].to_vec()
                    }
                } else {
                    if hi < 0 {
                        let l = list.len();
                        hi = max(0, l as i64 + hi);
                    }

                    if hi <= lo {
                        Vec::new()
                    } else {
                        list[lo as usize..hi.min(list.len() as i64) as usize].to_vec()
                    }
                }
            }
        };

        return Ok(DynamicValue::from(sublist));
    }

    let string = target.try_as_str()?;

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
                let l = string.chars().count();
                lo = max(0, l as i64 + lo);

                if hi < 0 {
                    hi = max(0, l as i64 + hi);
                }

                if hi <= lo {
                    "".to_string()
                } else {
                    chars.skip(lo as usize).take((hi - lo) as usize).collect()
                }
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
        DynamicValue::Bytes(bytes) => {
            let text =
                std::str::from_utf8(bytes).map_err(|_| EvaluationError::UnicodeDecodeError)?;

            match arg2 {
                DynamicValue::Regex(pattern) => Ok(DynamicValue::from(pattern.is_match(text))),
                _ => {
                    let pattern = arg2.try_as_str()?;
                    Ok(DynamicValue::from(text.contains(pattern.as_ref())))
                }
            }
        }
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

            Ok(DynamicValue::from(map.contains_key(needle.as_ref())))
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

fn index_by(args: BoundArguments) -> FunctionResult {
    let list = args.get1().try_as_list()?;
    let key = args.get(1).unwrap().try_as_str()?;

    let mut map: HashMap<String, DynamicValue> = HashMap::new();

    for item in list {
        let record = item.try_as_map()?;

        if let Some(value) = record.get(key.as_ref()) {
            map.insert(value.try_as_str()?.into_owned(), item.clone());
        }
    }

    Ok(DynamicValue::from(map))
}

// Arithmetics
fn parse_int(args: BoundArguments) -> FunctionResult {
    args.get1().try_as_i64().map(DynamicValue::from)
}

fn parse_float(args: BoundArguments) -> FunctionResult {
    args.get1().try_as_f64().map(DynamicValue::from)
}

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

fn round_like_op<F>(mut args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber) -> DynamicNumber,
{
    if args.len() == 2 {
        let unit = args.pop1_number()?;
        let operand = args.pop1_number()?;

        let result = op(operand / unit) * unit;

        Ok(DynamicValue::from(result))
    } else {
        Ok(DynamicValue::from(op(args.pop1_number()?)))
    }
}

fn binary_arithmetic_op<F>(args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber, DynamicNumber) -> DynamicNumber,
{
    let (n1, n2) = args.get2_number()?;

    Ok(DynamicValue::from(op(n1, n2)))
}

fn variadic_optimum<F, V, T>(args: BoundArguments, convert: F, validate: V) -> FunctionResult
where
    F: Fn(&DynamicValue) -> Result<T, EvaluationError>,
    V: Fn(Ordering) -> bool,
    T: Ord,
    DynamicValue: From<T>,
{
    if args.len() == 1 {
        let values = args.get1().try_as_list()?;

        if values.is_empty() {
            return Ok(DynamicValue::None);
        }

        let mut values_iter = values.iter();
        let mut best_value = convert(values_iter.next().unwrap())?;

        for value in values_iter {
            let other = convert(value)?;

            if validate(other.cmp(&best_value)) {
                best_value = other;
            }
        }

        return Ok(DynamicValue::from(best_value));
    }

    let mut args_iter = args.into_iter();
    let mut best_value = convert(&args_iter.next().unwrap())?;

    for arg in args_iter {
        let other_value = convert(&arg)?;

        if validate(other_value.cmp(&best_value)) {
            best_value = other_value;
        }
    }

    Ok(DynamicValue::from(best_value))
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

// Boolean
fn not(mut args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(!args.pop1_bool()))
}

// Comparison
fn abstract_compare<F>(mut args: BoundArguments, validate: F) -> FunctionResult
where
    F: FnOnce(Ordering) -> bool,
{
    let (a, b) = args.pop2();

    // TODO...
    let ordering = match (a, b) {
        // (DynamicValue::DateTime(a), b) => (*a).partial_cmp(&b.try_into_datetime()?),
        // (a, DynamicValue::DateTime(b)) => a.try_into_datetime()?.partial_cmp(&b),
        (a, b) => a.try_as_number()?.partial_cmp(&b.try_as_number()?),
    };

    Ok(DynamicValue::from(match ordering {
        Some(ordering) => validate(ordering),
        None => false,
    }))
}

fn sequence_compare<F>(args: BoundArguments, validate: F) -> FunctionResult
where
    F: FnOnce(Ordering) -> bool,
{
    // TODO: deal with lists
    let ordering = match args.get2() {
        (DynamicValue::Bytes(b1), DynamicValue::Bytes(b2)) => b1.partial_cmp(b2),
        (DynamicValue::String(s1), DynamicValue::String(s2)) => s1.partial_cmp(s2),
        (DynamicValue::Bytes(b1), DynamicValue::String(s2)) => std::str::from_utf8(b1)
            .map_err(|_| EvaluationError::UnicodeDecodeError)?
            .partial_cmp(s2.as_str()),
        (DynamicValue::String(s1), DynamicValue::Bytes(b2)) => s1
            .as_str()
            .partial_cmp(std::str::from_utf8(b2).map_err(|_| EvaluationError::UnicodeDecodeError)?),
        (u1, u2) => u1.try_as_str()?.partial_cmp(&u2.try_as_str()?),
    };

    Ok(DynamicValue::from(match ordering {
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

fn sum(args: BoundArguments) -> FunctionResult {
    let items = args.get1().try_as_list()?;
    let mut sum = Sum::new();

    for item in items {
        sum.add(item.try_as_number()?);
    }

    Ok(DynamicValue::from(sum.get()))
}

// Introspection
fn type_of(mut args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(args.pop1().type_of()))
}

// Random
fn uuid(_args: BoundArguments) -> FunctionResult {
    let id = Uuid::new_v4().to_string();

    Ok(DynamicValue::from(id))
}

fn random(_args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(rand::rng().random::<f64>()))
}

// Fuzzy matching
lazy_static! {
    static ref FINGERPRINT_TOKENIZER: FingerprintTokenizer = FingerprintTokenizer::default();
}

fn fingerprint(args: BoundArguments) -> FunctionResult {
    let string = args.get1().try_as_str()?;

    Ok(DynamicValue::from(
        FINGERPRINT_TOKENIZER.key(string.as_ref()),
    ))
}

fn abstract_unary_string_fn<F>(args: BoundArguments, function: F) -> FunctionResult
where
    F: FnOnce(&str) -> Cow<str>,
{
    let string = args.get1().try_as_str()?;

    Ok(DynamicValue::from(function(&string)))
}

// Utils
fn err(args: BoundArguments) -> FunctionResult {
    let arg = args.get1_str()?;

    Err(EvaluationError::Custom(arg.to_string()))
}

fn parse_json(args: BoundArguments) -> FunctionResult {
    let arg = args.get1();

    serde_json::from_slice(arg.try_as_bytes()?)
        .map_err(|_| EvaluationError::JSONParseError(format!("{:?}", args.get1())))
}

fn parse_py_literal(args: BoundArguments) -> FunctionResult {
    let parsed: py_literal::Value = args
        .get1_str()?
        .parse()
        .map_err(|err: py_literal::ParseError| EvaluationError::Custom(err.to_string()))?;

    fn map_to_dynamic_value(value: py_literal::Value) -> FunctionResult {
        Ok(match value {
            py_literal::Value::None => DynamicValue::None,
            py_literal::Value::Boolean(v) => DynamicValue::Boolean(v),
            py_literal::Value::Float(f) => DynamicValue::Float(f),
            py_literal::Value::Integer(bi) => match bi.try_into() {
                Ok(i) => DynamicValue::Integer(i),
                Err(err) => return Err(EvaluationError::Custom(err.to_string())),
            },
            py_literal::Value::Bytes(b) => DynamicValue::from_owned_bytes(b),
            py_literal::Value::String(s) => DynamicValue::from(s),
            py_literal::Value::List(l)
            | py_literal::Value::Tuple(l)
            | py_literal::Value::Set(l) => {
                let mut list = Vec::new();

                for item in l {
                    list.push(map_to_dynamic_value(item)?);
                }

                DynamicValue::from(list)
            }
            py_literal::Value::Dict(d) => {
                let mut dict = HashMap::new();

                for (key, value) in d {
                    dict.insert(
                        map_to_dynamic_value(key)?.try_as_str()?.into_owned(),
                        map_to_dynamic_value(value)?,
                    );
                }

                DynamicValue::from(dict)
            }
            py_literal::Value::Complex(c) => {
                DynamicValue::from(vec![DynamicValue::Float(c.re), DynamicValue::Float(c.im)])
            }
        })
    }

    map_to_dynamic_value(parsed)
}

fn parse_dataurl(args: BoundArguments) -> FunctionResult {
    let bytes = args.get1().try_as_bytes()?;

    if !bytes.starts_with(b"data:") {
        return Err(EvaluationError::Custom(
            "data url does not start with \"data:\"".to_string(),
        ));
    }

    match bytes[5..].split_once_str(b",") {
        Some((spec, data)) => {
            if spec.ends_with(b";base64") {
                let mime = &spec[..spec.len() - 7];

                Ok(DynamicValue::from(vec![
                    DynamicValue::from(std::str::from_utf8(mime).unwrap()),
                    DynamicValue::from_owned_bytes(BASE64_STANDARD.decode(data).map_err(|_| {
                        EvaluationError::Custom("data url contains invalid base64".to_string())
                    })?),
                ]))
            } else {
                Err(EvaluationError::NotImplemented(
                    "url-encoded data url is not implemented yet".to_string(),
                ))
            }
        }
        None => Err(EvaluationError::Custom(
            "data url is misformatted".to_string(),
        )),
    }
}

fn mime_ext(args: BoundArguments) -> FunctionResult {
    let target = args.get1_str()?;

    match mime2ext(target) {
        Some(ext) => Ok(DynamicValue::from(ext)),
        None => Err(EvaluationError::Custom("unknown MIME type".to_string())),
    }
}

fn html_unescape(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;

    Ok(DynamicValue::from(html_escape::decode_html_entities(
        &string,
    )))
}

fn parse_regex(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;

    Ok(DynamicValue::from(Regex::new(&string).map_err(|_| {
        EvaluationError::Custom(format!("could not parse \"{}\" as regex", string))
    })?))
}
