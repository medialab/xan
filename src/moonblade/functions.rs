use std::borrow::Cow;
use std::cmp::{max, Ordering, PartialOrd};
use std::fs::{self, File};
use std::io::Read;
use std::ops::{Add, Div, Mul, Neg, Rem, Sub};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use base64::prelude::*;
use bstr::ByteSlice;
use bytesize::ByteSize;
use encoding::{label::encoding_from_whatwg_label, DecoderTrap};
use flate2::read::MultiGzDecoder;
use jiff::{fmt::strtime, tz::TimeZone, Timestamp, Zoned};
use lazy_static::lazy_static;
use mime2ext::mime2ext;
use namedlock::{AutoCleanup, LockSpace};
use paltoquet::{
    stemmers::{fr::carry_stemmer, s_stemmer},
    tokenizers::FingerprintTokenizer,
};
use rand::Rng;
use regex::Regex;
use unidecode::unidecode;
use uuid::Uuid;

use crate::collections::HashMap;
use crate::dates;
use crate::urls::LRUStems;

use super::agg::aggregators::{Sum, Welford};
use super::error::EvaluationError;
use super::types::{Argument, BoundArguments, DynamicNumber, DynamicValue, FunctionArguments};

type FunctionResult = Result<DynamicValue, EvaluationError>;
pub type Function = fn(BoundArguments) -> FunctionResult;

pub fn get_function(name: &str) -> Option<(Function, FunctionArguments)> {
    Some(match name {
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
        "abs" => (
            |args| unary_arithmetic_op(args, DynamicNumber::abs),
            FunctionArguments::unary(),
        ),
        "abspath" => (abspath, FunctionArguments::unary()),
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
        "basename" => (basename, FunctionArguments::with_range(1..=2)),
        "bytesize" => (bytesize, FunctionArguments::unary()),
        "carry_stemmer" => (carry_stemmer_fn, FunctionArguments::unary()),
        "ceil" => (
            |args| round_like_op(args, DynamicNumber::ceil),
            FunctionArguments::with_range(1..=2),
        ),
        "cmd" => (cmd, FunctionArguments::binary()),
        "compact" => (compact, FunctionArguments::unary()),
        "concat" => (concat, FunctionArguments::variadic(2)),
        "contains" => (contains, FunctionArguments::binary()),
        "copy" => (copy_file, FunctionArguments::binary()),
        "count" => (count, FunctionArguments::binary()),
        "datetime" => (
            datetime,
            FunctionArguments::complex(vec![
                Argument::Positional,
                Argument::with_name("format"),
                Argument::with_name("timezone"),
            ]),
        ),
        "dirname" => (dirname, FunctionArguments::unary()),
        "div" => (
            |args| variadic_arithmetic_op(args, Div::div),
            FunctionArguments::variadic(2),
        ),
        "earliest" => (
            |args| {
                variadic_optimum(
                    args,
                    |value| value.try_as_datetime().map(Cow::into_owned),
                    Ordering::is_lt,
                )
            },
            FunctionArguments::variadic(1),
        ),
        "endswith" => (endswith, FunctionArguments::binary()),
        "err" => (err, FunctionArguments::unary()),
        "escape_regex" => (escape_regex, FunctionArguments::unary()),
        "ext" => (ext, FunctionArguments::unary()),
        "filesize" => (filesize, FunctionArguments::unary()),
        "fingerprint" => (fingerprint, FunctionArguments::unary()),
        "first" => (first, FunctionArguments::unary()),
        "float" => (parse_float, FunctionArguments::unary()),
        "floor" => (
            |args| round_like_op(args, DynamicNumber::floor),
            FunctionArguments::with_range(1..=2),
        ),
        "fmt" => (fmt, FunctionArguments::variadic(2)),
        "numfmt" => (
            fmt_number,
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
        "isfile" => (isfile, FunctionArguments::unary()),
        "join" => (join, FunctionArguments::binary()),
        "keys" => (keys, FunctionArguments::unary()),
        "latest" => (
            |args| {
                variadic_optimum(
                    args,
                    |value| value.try_as_datetime().map(Cow::into_owned),
                    Ordering::is_gt,
                )
            },
            FunctionArguments::variadic(1),
        ),
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
        "lru" => (lru, FunctionArguments::unary()),
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
            |args| custom_strftime(args, "%m"),
            FunctionArguments::unary(),
        ),
        "month_day" => (
            |args| custom_strftime(args, "%m-%d"),
            FunctionArguments::unary(),
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
        "pad" => (
            |args| pad(pad::Alignment::Middle, args),
            FunctionArguments::with_range(2..=3),
        ),
        "lpad" => (
            |args| pad(pad::Alignment::Right, args),
            FunctionArguments::with_range(2..=3),
        ),
        "rpad" => (
            |args| pad(pad::Alignment::Left, args),
            FunctionArguments::with_range(2..=3),
        ),
        "parse_dataurl" => (parse_dataurl, FunctionArguments::unary()),
        "parse_json" => (parse_json, FunctionArguments::unary()),
        "parse_py_literal" => (parse_py_literal, FunctionArguments::unary()),
        "pjoin" | "pathjoin" => (pathjoin, FunctionArguments::variadic(2)),
        "pow" => (
            |args| binary_arithmetic_op(args, DynamicNumber::pow),
            FunctionArguments::binary(),
        ),
        "printf" => (printf, FunctionArguments::variadic(2)),
        "random" => (random, FunctionArguments::nullary()),
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
        "regex" => (parse_regex, FunctionArguments::unary()),
        "replace" => (replace, FunctionArguments::nary(3)),
        "round" => (
            |args| round_like_op(args, DynamicNumber::round),
            FunctionArguments::with_range(1..=2),
        ),
        "shell" => (shell, FunctionArguments::unary()),
        "shlex_split" => (shlex_split, FunctionArguments::unary()),
        "slice" => (slice, FunctionArguments::with_range(2..=3)),
        "split" => (split, FunctionArguments::with_range(2..=3)),
        "sqrt" => (
            |args| unary_arithmetic_op(args, DynamicNumber::sqrt),
            FunctionArguments::unary(),
        ),
        "startswith" => (startswith, FunctionArguments::binary()),
        "strftime" => (
            strftime,
            FunctionArguments::complex(vec![
                Argument::Positional,
                Argument::Positional,
                Argument::with_name("timezone"),
            ]),
        ),
        "sub" => (
            |args| variadic_arithmetic_op(args, Sub::sub),
            FunctionArguments::variadic(2),
        ),
        "sum" => (sum, FunctionArguments::unary()),
        "s_stemmer" => (s_stemmer_fn, FunctionArguments::unary()),
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
        "timestamp" => (timestamp, FunctionArguments::unary()),
        "timestamp_ms" => (timestamp_ms, FunctionArguments::unary()),
        "to_fixed" => (to_fixed, FunctionArguments::binary()),
        "to_timezone" => (to_timezone, FunctionArguments::nary(3)),
        "to_local_timezone" => (to_local_timezone, FunctionArguments::binary()),
        "trim" => (trim, FunctionArguments::with_range(1..=2)),
        "ltrim" => (ltrim, FunctionArguments::with_range(1..=2)),
        "rtrim" => (rtrim, FunctionArguments::with_range(1..=2)),
        "trunc" => (
            |args| round_like_op(args, DynamicNumber::trunc),
            FunctionArguments::with_range(1..=2),
        ),
        "typeof" => (type_of, FunctionArguments::unary()),
        "unidecode" => (apply_unidecode, FunctionArguments::unary()),
        "upper" => (upper, FunctionArguments::unary()),
        "urljoin" => (urljoin, FunctionArguments::binary()),
        "uuid" => (uuid, FunctionArguments::nullary()),
        "values" => (values, FunctionArguments::unary()),
        "write" => (write, FunctionArguments::binary()),
        "year" => (
            |args| custom_strftime(args, "%Y"),
            FunctionArguments::unary(),
        ),
        "year_month_day" | "ymd" => (
            |args| custom_strftime(args, "%F"),
            FunctionArguments::unary(),
        ),
        "year_month" | "ym" => (
            |args| custom_strftime(args, "%Y-%m"),
            FunctionArguments::unary(),
        ),
        _ => return None,
    })
}

// Strings
macro_rules! make_trim_fn {
    ($name: ident, $trim: ident, $trim_matches: ident) => {
        fn $name(args: BoundArguments) -> FunctionResult {
            let chars_opt = args.get(1);

            Ok(match chars_opt {
                None => match args.get1() {
                    DynamicValue::Bytes(bytes) => DynamicValue::from(bytes.$trim()),
                    value => DynamicValue::from(value.try_as_str()?.$trim()),
                },
                Some(chars) => {
                    let pattern = chars.try_as_str()?.chars().collect::<Vec<char>>();
                    DynamicValue::from(args.get1_str()?.$trim_matches(|c| pattern.contains(&c)))
                }
            })
        }
    };
}

make_trim_fn!(trim, trim, trim_matches);
make_trim_fn!(ltrim, trim_start, trim_start_matches);
make_trim_fn!(rtrim, trim_end, trim_end_matches);

fn pad(alignment: pad::Alignment, args: BoundArguments) -> FunctionResult {
    use pad::PadStr;

    let mut args_iter = args.into_iter();
    let first_arg = args_iter.next().unwrap();
    let string = first_arg.try_as_str()?;

    let width = args_iter.next().unwrap().try_as_usize()?;
    let padding_char = match args_iter.next() {
        None => ' ',
        Some(value) => {
            let padding_string = value.try_as_str()?;

            match padding_string.chars().count() {
                0 => {
                    return Err(EvaluationError::Custom(
                        "provided padding char is empty".to_string(),
                    ));
                }
                1 => padding_string.chars().next().unwrap(),
                2.. => {
                    return Err(EvaluationError::Custom(
                        "provided padding char is longer than a char".to_string(),
                    ));
                }
            }
        }
    };

    Ok(DynamicValue::from(string.pad(
        width,
        padding_char,
        alignment,
        false,
    )))
}

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

lazy_static! {
    static ref FMT_PATTERN: regex::Regex = regex::Regex::new(r"\{([A-Za-z_]*)\}").unwrap();
}

fn fmt(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let first_arg = args_iter.next().unwrap();
    let mut rest = args_iter.collect::<Vec<_>>();
    let substitution_map = if rest.len() == 1 {
        match rest.pop().unwrap() {
            DynamicValue::Map(map) => Some(map),
            other => {
                rest.push(other);
                None
            }
        }
    } else {
        None
    };

    let pattern = first_arg.try_as_str()?;

    let mut formatted = String::with_capacity(pattern.len());
    let mut current_positional: usize = 0;
    let mut last_match = 0;

    for capture in FMT_PATTERN.captures_iter(&pattern) {
        let m = capture.get(0).unwrap();
        let fallback = &capture[0];

        formatted.push_str(&pattern[last_match..m.start()]);

        match capture.get(1).unwrap().as_str() {
            "" => {
                if current_positional < rest.len() {
                    formatted.push_str(&rest[current_positional].try_as_str()?);
                    current_positional += 1;
                } else {
                    formatted.push_str(fallback);
                }
            }
            key => {
                if let Some(map) = &substitution_map {
                    if let Some(sub) = map.get(key) {
                        formatted.push_str(&sub.try_as_str()?);
                    } else {
                        formatted.push_str(fallback);
                    }
                } else {
                    formatted.push_str(fallback);
                }
            }
        };

        last_match = m.end();
    }

    formatted.push_str(&pattern[last_match..]);

    Ok(DynamicValue::from(formatted))
}

fn fmt_number(args: BoundArguments) -> FunctionResult {
    let mut args_iter = args.into_iter();
    let number = args_iter.next().unwrap().try_as_number()?;

    let thousands_sep = args_iter.next().unwrap();
    let comma = args_iter.next().unwrap();
    let significance = args_iter.next().unwrap();

    if !thousands_sep.is_none() || !comma.is_none() || !significance.is_none() {
        let mut formatter = numfmt::Formatter::new()
            .separator(',')
            .unwrap()
            .comma(comma.is_truthy());

        let separator = if comma.is_truthy() { '.' } else { ',' };

        if !significance.is_none() {
            formatter = formatter.precision(numfmt::Precision::Significance(
                significance.try_as_usize()? as u8,
            ));
        } else {
            formatter = formatter.precision(numfmt::Precision::Significance(5));
        }

        let mut formatted = crate::util::format_number_with_formatter(&mut formatter, number);

        if !thousands_sep.is_none() {
            formatted = formatted.replace(separator, &thousands_sep.try_as_str()?);
        }

        Ok(DynamicValue::from(formatted))
    } else {
        Ok(DynamicValue::from(crate::util::format_number(number)))
    }
}

fn printf(args: BoundArguments) -> FunctionResult {
    let l = args.len() - 1;

    let mut args_iter = args.into_iter();
    let fmt_arg = args_iter.next().unwrap();
    let fmt = fmt_arg.try_as_str()?;

    let mut fmt_args: Vec<Box<dyn sprintf::Printf>> = Vec::with_capacity(l);

    fn arg_to_printf(arg: &DynamicValue) -> Result<Box<dyn sprintf::Printf>, EvaluationError> {
        Ok(match arg {
            DynamicValue::Integer(i) => Box::new(*i),
            DynamicValue::Float(f) => Box::new(*f),
            _ => Box::new(arg.try_as_str()?.into_owned()),
        })
    }

    for arg in args_iter {
        if let DynamicValue::List(list) = arg {
            for sub_arg in list.iter() {
                fmt_args.push(arg_to_printf(sub_arg)?);
            }
        } else {
            fmt_args.push(arg_to_printf(&arg)?);
        }
    }

    let fmt_args_refs = fmt_args.iter().map(|b| b.as_ref()).collect::<Vec<_>>();

    match sprintf::vsprintf(&fmt, &fmt_args_refs) {
        Ok(string) => Ok(DynamicValue::from(string)),
        Err(error) => Err(EvaluationError::Custom(error.to_string())),
    }
}

fn to_fixed(mut args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.pop2();

    let n = arg1.try_as_f64()?;
    let p = arg2.try_as_usize()?.min(16);

    let formatted = format!("{:.precision$}", n, precision = p);

    Ok(DynamicValue::from(formatted))
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

    let ordering = match (a, b) {
        (DynamicValue::DateTime(a), b) => (*a).partial_cmp(&b.try_into_datetime()?),
        (a, DynamicValue::DateTime(b)) => a.try_into_datetime()?.partial_cmp(&b),
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
                let mut gz = MultiGzDecoder::new(file);
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
                let mut gz = MultiGzDecoder::new(file);
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
    serde_json::from_str(&contents)
        .map_err(|_| EvaluationError::JSONParseError(format!("{:?}", contents)))
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
    let data = args.get1();
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

    fs::write(&path, data.try_as_bytes()?).map_err(|_| {
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
    let string = args.get1_str()?;
    let path = Path::new(string.as_ref());

    Ok(DynamicValue::from(
        path.extension().and_then(|e| e.to_str()),
    ))
}

fn dirname(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;
    let path = Path::new(string.as_ref());

    Ok(DynamicValue::from(path.parent().and_then(|p| p.to_str())))
}

fn basename(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;
    let path = Path::new(string.as_ref());

    let name = path.file_name().and_then(|p| p.to_str());

    if args.len() == 2 {
        let suffix = args.get(1).unwrap().try_as_str()?;

        Ok(DynamicValue::from(
            name.and_then(|n| n.strip_suffix(suffix.as_ref()).or(Some(n))),
        ))
    } else {
        Ok(DynamicValue::from(name))
    }
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
    let human_readable = ByteSize::b(bytes).display().si().to_string();

    Ok(DynamicValue::from(human_readable))
}

// Dates
fn timestamp(args: BoundArguments) -> FunctionResult {
    let seconds = args.get1().try_as_i64()?;
    let utc = TimeZone::UTC;
    match Timestamp::from_second(seconds) {
        Ok(timestamp) => Ok(DynamicValue::from(timestamp.to_zoned(utc))),
        Err(_) => Err(EvaluationError::DateTime(format!(
            "cannot parse \"{}\" as timestamp",
            seconds
        ))),
    }
}

fn timestamp_ms(args: BoundArguments) -> FunctionResult {
    let milliseconds = args.get1().try_as_i64()?;
    let utc = TimeZone::UTC;
    match Timestamp::from_millisecond(milliseconds) {
        Ok(timestamp) => Ok(DynamicValue::from(timestamp.to_zoned(utc))),
        Err(_) => Err(EvaluationError::DateTime(format!(
            "cannot parse \"{}\" as timestamp",
            milliseconds
        ))),
    }
}

fn datetime(args: BoundArguments) -> FunctionResult {
    let datestring = args.get1().try_as_str()?;
    let format = args.get_not_none(1).map(|f| f.try_as_str()).transpose()?;
    let timezone = args.get_not_none(2);

    dates::parse_zoned(
        &datestring,
        format.as_deref(),
        timezone.map(|tz| tz.try_as_timezone()).transpose()?,
    )
    .map_err(|err| {
        EvaluationError::from_zoned_parse_error(
            &datestring,
            format.as_deref(),
            timezone.map(|tz| tz.try_as_str().unwrap()).as_deref(),
            err,
        )
    })
    .map(DynamicValue::from)
}

fn to_timezone(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2, arg3) = args.get3();
    // We could check if arg1 is a datetime before parsing it as str
    let datestring = arg1.try_as_str()?;
    let timezone_in = arg2.try_as_timezone()?;
    let timezone_out = arg3.try_as_timezone()?;

    dates::parse_zoned(&datestring, None, Some(timezone_in))
        .map_err(|err| {
            EvaluationError::from_zoned_parse_error(
                &datestring,
                None,
                Some(arg2.try_as_str().unwrap().as_ref()),
                err,
            )
        })
        .map(|dt| DynamicValue::from(dt.with_time_zone(timezone_out)))
}

fn to_local_timezone(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();
    // We could check if arg1 is a datetime before parsing it as str
    let datestring = arg1.try_as_str()?;
    let timezone_in = arg2.try_as_timezone()?;

    dates::parse_zoned(&datestring, None, Some(timezone_in))
        .map_err(|err| {
            EvaluationError::from_zoned_parse_error(
                &datestring,
                None,
                Some(arg2.try_as_str().unwrap().as_ref()),
                err,
            )
        })
        .map(|dt| DynamicValue::from(dt.with_time_zone(TimeZone::system())))
}

fn abstract_strftime(datetime: &Zoned, format: &str) -> FunctionResult {
    match strtime::format(format, datetime) {
        Ok(formatted) => Ok(DynamicValue::from(formatted)),
        Err(_) => Err(EvaluationError::DateTime(format!(
            "\"{}\" is not a valid format",
            format
        ))),
    }
}

fn strftime(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();
    let datetime = arg1.try_as_datetime()?;
    let format = arg2.try_as_str()?;

    abstract_strftime(&datetime, &format)
}

fn custom_strftime(args: BoundArguments, format: &str) -> FunctionResult {
    let target = args.get1();
    let datetime = target.try_as_datetime()?;

    abstract_strftime(&datetime, format)
}

// Urls
fn urljoin(args: BoundArguments) -> FunctionResult {
    let mut url = args.get(0).unwrap().try_as_url()?;
    let addendum = args.get(1).unwrap().try_as_str()?;

    url = url
        .join(&addendum)
        .map_err(|_| EvaluationError::Custom("invalid url part to join".to_string()))?;

    // TODO: canonicalize
    Ok(DynamicValue::from(url.to_string()))
}

fn lru(args: BoundArguments) -> FunctionResult {
    let tagged_url = args.get1().try_as_tagged_url()?;

    Ok(DynamicValue::from(LRUStems::from(&tagged_url).to_string()))
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

fn s_stemmer_fn(args: BoundArguments) -> FunctionResult {
    let string = args.get1().try_as_str()?;

    Ok(DynamicValue::from(s_stemmer(&string)))
}

fn carry_stemmer_fn(args: BoundArguments) -> FunctionResult {
    let string = args.get1().try_as_str()?;

    Ok(DynamicValue::from(carry_stemmer(&string)))
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

fn shlex_split(args: BoundArguments) -> FunctionResult {
    let string = args.get1_str()?;

    if let Some(splitted) = shlex::split(&string) {
        Ok(DynamicValue::from(
            splitted
                .into_iter()
                .map(DynamicValue::from)
                .collect::<Vec<_>>(),
        ))
    } else {
        Err(EvaluationError::Custom(format!(
            "could not split {:?}",
            args.get1()
        )))
    }
}

fn cmd(mut args: BoundArguments) -> FunctionResult {
    let (command_name_arg, command_args) = args.pop2();

    let command_name = command_name_arg.try_as_str()?;

    let mut command = Command::new(command_name.as_ref());

    for command_arg in command_args.try_as_list()? {
        command.arg(command_arg.try_as_str()?.as_ref());
    }

    if let Ok(mut output) = command.output() {
        if output.status.success() {
            let result = &mut output.stdout;
            result.truncate(result.trim_ascii_end().len());

            Ok(DynamicValue::from_owned_bytes(output.stdout))
        } else {
            Err(EvaluationError::Custom(format!(
                "\"{}\" failed!",
                command_name
            )))
        }
    } else {
        Err(EvaluationError::Custom(format!(
            "error while spawning \"{}\"",
            command_name
        )))
    }
}

fn shell(args: BoundArguments) -> FunctionResult {
    let pipeline = args.get1_str()?;

    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", pipeline.as_ref()]);
        command
    } else {
        let mut command = Command::new(std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string()));
        command.args(["-c", pipeline.as_ref()]);
        command
    };

    if let Ok(mut output) = command.output() {
        if output.status.success() {
            let result = &mut output.stdout;
            result.truncate(result.trim_ascii_end().len());

            Ok(DynamicValue::from_owned_bytes(output.stdout))
        } else {
            Err(EvaluationError::Custom(format!(
                "shell pipeline \"{}\" failed!",
                pipeline
            )))
        }
    } else {
        Err(EvaluationError::Custom(format!(
            "error while running shell pipeline \"{}\"",
            pipeline
        )))
    }
}
