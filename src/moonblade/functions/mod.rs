use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops::{Div, Mul, Neg, Rem};

use paltoquet::{
    phonetics::{phonogram, refined_soundex, soundex},
    stemmers::{fr::carry_stemmer, s_stemmer},
};

use super::error::EvaluationError;
use super::types::{Argument, BoundArguments, DynamicNumber, DynamicValue, FunctionArguments};

mod fmt;
mod fuzzy;
// mod io;
mod maps;
mod ops;
// mod sequences;
pub mod special;
// mod strings;
// mod temporal;
mod utils;
// mod web;

pub type FunctionResult = Result<DynamicValue, EvaluationError>;
pub type Function = fn(BoundArguments) -> FunctionResult;

pub fn get_function(name: &str) -> Option<(Function, FunctionArguments)> {
    Some(match name {
        // Operators
        // "==" => (
        //     |args| ops::abstract_compare(args, Ordering::is_eq),
        //     FunctionArguments::binary(),
        // ),
        // ">" => (
        //     |args| ops::abstract_compare(args, Ordering::is_gt),
        //     FunctionArguments::binary(),
        // ),
        // ">=" => (
        //     |args| ops::abstract_compare(args, Ordering::is_ge),
        //     FunctionArguments::binary(),
        // ),
        // "<" => (
        //     |args| ops::abstract_compare(args, Ordering::is_lt),
        //     FunctionArguments::binary(),
        // ),
        // "<=" => (
        //     |args| ops::abstract_compare(args, Ordering::is_le),
        //     FunctionArguments::binary(),
        // ),
        // "!=" => (
        //     |args| ops::abstract_compare(args, Ordering::is_ne),
        //     FunctionArguments::binary(),
        // ),
        // "eq" => (
        //     |args| ops::sequence_compare(args, Ordering::is_eq),
        //     FunctionArguments::binary(),
        // ),
        // "gt" => (
        //     |args| ops::sequence_compare(args, Ordering::is_gt),
        //     FunctionArguments::binary(),
        // ),
        // "ge" => (
        //     |args| ops::sequence_compare(args, Ordering::is_ge),
        //     FunctionArguments::binary(),
        // ),
        // "lt" => (
        //     |args| ops::sequence_compare(args, Ordering::is_lt),
        //     FunctionArguments::binary(),
        // ),
        // "le" => (
        //     |args| ops::sequence_compare(args, Ordering::is_le),
        //     FunctionArguments::binary(),
        // ),
        // "ne" => (
        //     |args| ops::sequence_compare(args, Ordering::is_ne),
        //     FunctionArguments::binary(),
        // ),

        // Functions
        "abs" => (
            |args| ops::unary_arithmetic_op(args, DynamicNumber::abs),
            FunctionArguments::unary(),
        ),
        // "abspath" => (io::abspath, FunctionArguments::unary()),
        // "add" => (ops::add, FunctionArguments::variadic(2)),
        // "argmax" => (
        //     |args| ops::argcompare(args, Ordering::is_gt),
        //     FunctionArguments::with_range(1..=2),
        // ),
        // "argmin" => (
        //     |args| ops::argcompare(args, Ordering::is_lt),
        //     FunctionArguments::with_range(1..=2),
        // ),
        // "basename" => (io::basename, FunctionArguments::with_range(1..=2)),
        // "bytesize" => (io::bytesize, FunctionArguments::unary()),
        // "carry_stemmer" => (
        //     |args| ops::abstract_unary_string_fn(args, |string| Cow::Owned(carry_stemmer(string))),
        //     FunctionArguments::unary(),
        // ),
        // "ceil" => (
        //     |args| ops::round_like_op(args, DynamicNumber::ceil),
        //     FunctionArguments::with_range(1..=2),
        // ),
        // "cmd" => (io::cmd, FunctionArguments::binary()),
        // "compact" => (sequences::compact, FunctionArguments::unary()),
        // "concat" => (sequences::concat, FunctionArguments::variadic(2)),
        "contains" => (maps::contains, FunctionArguments::binary()),
        // "copy" => (io::copy_file, FunctionArguments::binary()),
        // "count" => (strings::count, FunctionArguments::binary()),
        // "date" => (temporal::date, FunctionArguments::with_range(1..=2)),
        // "datetime" => (temporal::datetime, FunctionArguments::with_range(1..=2)),
        // "dirname" => (io::dirname, FunctionArguments::unary()),
        // "div" => (
        //     |args| ops::variadic_arithmetic_op(args, Div::div),
        //     FunctionArguments::variadic(2),
        // ),
        // "earliest" => (
        //     |args| ops::variadic_optimum(args, DynamicValue::try_as_any_temporal, Ordering::is_lt),
        //     FunctionArguments::variadic(1),
        // ),
        // "endswith" => (strings::endswith, FunctionArguments::binary()),
        "err" => (utils::err, FunctionArguments::unary()),
        "escape_regex" => (fmt::escape_regex, FunctionArguments::unary()),
        // "ext" => (io::ext, FunctionArguments::unary()),
        // "filesize" => (io::filesize, FunctionArguments::unary()),
        "fingerprint" => (fuzzy::fingerprint, FunctionArguments::unary()),
        // "first" => (sequences::first, FunctionArguments::unary()),
        "float" => (ops::parse_float, FunctionArguments::unary()),
        // "floor" => (
        //     |args| ops::round_like_op(args, DynamicNumber::floor),
        //     FunctionArguments::with_range(1..=2),
        // ),
        "fmt" => (fmt::fmt, FunctionArguments::variadic(2)),
        // "fractional_days" => (temporal::fractional_days, FunctionArguments::binary()),
        // "from_timestamp" => (temporal::from_timestamp, FunctionArguments::unary()),
        // "from_timestamp_ms" => (temporal::from_timestamp_ms, FunctionArguments::unary()),
        "get" => (maps::get, FunctionArguments::with_range(2..=3)),
        // "html_unescape" => (web::html_unescape, FunctionArguments::unary()),
        // "idiv" => (
        //     |args| ops::arithmetic_op(args, DynamicNumber::idiv),
        //     FunctionArguments::binary(),
        // ),
        "index_by" => (maps::index_by, FunctionArguments::binary()),
        "int" => (ops::parse_int, FunctionArguments::unary()),
        // "isfile" => (io::isfile, FunctionArguments::unary()),
        // "join" => (strings::join, FunctionArguments::binary()),
        "keys" => (maps::keys, FunctionArguments::unary()),
        // "latest" => (
        //     |args| ops::variadic_optimum(args, DynamicValue::try_as_any_temporal, Ordering::is_gt),
        //     FunctionArguments::variadic(1),
        // ),
        // "last" => (sequences::last, FunctionArguments::unary()),
        // "len" => (sequences::len, FunctionArguments::unary()),
        // "log" => (
        //     |args| match args.len() {
        //         1 => ops::unary_arithmetic_op(args, DynamicNumber::ln),
        //         2 => ops::binary_arithmetic_op(args, DynamicNumber::log),
        //         _ => unreachable!(),
        //     },
        //     FunctionArguments::with_range(1..=2),
        // ),
        // "log2" => (
        //     |args| ops::unary_arithmetic_op(args, DynamicNumber::log2),
        //     FunctionArguments::unary(),
        // ),
        // "log10" => (
        //     |args| ops::unary_arithmetic_op(args, DynamicNumber::log10),
        //     FunctionArguments::unary(),
        // ),
        // "lower" => (strings::lower, FunctionArguments::unary()),
        // "lru" => (web::lru, FunctionArguments::unary()),
        // "match" => (strings::regex_match, FunctionArguments::with_range(2..=3)),
        // "max" => (
        //     |args| ops::variadic_optimum(args, DynamicValue::try_as_number, Ordering::is_gt),
        //     FunctionArguments::variadic(1),
        // ),
        "md5" => (utils::md5, FunctionArguments::unary()),
        // "mean" => (ops::mean, FunctionArguments::unary()),
        // "mime_ext" => (web::mime_ext, FunctionArguments::unary()),
        // "min" => (
        //     |args| ops::variadic_optimum(args, DynamicValue::try_as_number, Ordering::is_lt),
        //     FunctionArguments::variadic(1),
        // ),
        // "mod" => (
        //     |args| ops::binary_arithmetic_op(args, Rem::rem),
        //     FunctionArguments::binary(),
        // ),
        // "month" => (
        //     |args| temporal::custom_strftime(args, "%m"),
        //     FunctionArguments::unary(),
        // ),
        // "month_day" => (
        //     |args| temporal::custom_strftime(args, "%m-%d"),
        //     FunctionArguments::unary(),
        // ),
        // "move" => (io::move_file, FunctionArguments::binary()),
        // "mul" => (
        //     |args| ops::variadic_arithmetic_op(args, Mul::mul),
        //     FunctionArguments::variadic(2),
        // ),
        // "neg" => (
        //     |args| ops::unary_arithmetic_op(args, Neg::neg),
        //     FunctionArguments::unary(),
        // ),
        // "not" => (ops::not, FunctionArguments::unary()),
        // "now" => (temporal::now, FunctionArguments::nullary()),
        "numfmt" => (
            fmt::fmt_number,
            FunctionArguments::complex(vec![
                Argument::Positional,
                Argument::with_name("thousands_sep"),
                Argument::with_name("comma"),
                Argument::with_name("significance"),
            ]),
        ),
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
        // "parse_dataurl" => (web::parse_dataurl, FunctionArguments::unary()),
        // "parse_json" => (io::parse_json, FunctionArguments::unary()),
        // "parse_py_literal" => (io::parse_py_literal, FunctionArguments::unary()),
        // "phonogram" => (
        //     |args| ops::abstract_unary_string_fn(args, |string| Cow::Owned(phonogram(string))),
        //     FunctionArguments::unary(),
        // ),
        // "pjoin" | "pathjoin" => (io::pathjoin, FunctionArguments::variadic(2)),
        // "pow" => (
        //     |args| ops::binary_arithmetic_op(args, DynamicNumber::pow),
        //     FunctionArguments::binary(),
        // ),
        "printf" => (fmt::printf, FunctionArguments::variadic(2)),
        "random" => (utils::random, FunctionArguments::nullary()),
        // "range" => (sequences::range, FunctionArguments::with_range(1..=3)),
        // "read" => (
        //     io::read,
        //     FunctionArguments::complex(vec![
        //         Argument::Positional,
        //         Argument::with_name("encoding"),
        //         Argument::with_name("errors"),
        //     ]),
        // ),
        // "read_csv" => (io::read_csv, FunctionArguments::unary()),
        // "read_json" => (io::read_json, FunctionArguments::unary()),
        // "refined_soundex" => (
        //     |args| {
        //         ops::abstract_unary_string_fn(args, |string| Cow::Owned(refined_soundex(string)))
        //     },
        //     FunctionArguments::unary(),
        // ),
        "regex" => (utils::parse_regex, FunctionArguments::unary()),
        // "repeat" => (sequences::repeat, FunctionArguments::binary()),
        // "replace" => (strings::replace, FunctionArguments::nary(3)),
        // "round" => (
        //     |args| ops::round_like_op(args, DynamicNumber::round),
        //     FunctionArguments::with_range(1..=2),
        // ),
        // "shell" => (io::shell, FunctionArguments::unary()),
        // "shlex_split" => (io::shlex_split, FunctionArguments::unary()),
        // "slice" => (sequences::slice, FunctionArguments::with_range(2..=3)),
        // "soundex" => (
        //     |args| ops::abstract_unary_string_fn(args, |string| Cow::Owned(soundex(string))),
        //     FunctionArguments::unary(),
        // ),
        // "span" => (temporal::span, FunctionArguments::unary()),
        // "split" => (strings::split, FunctionArguments::with_range(2..=3)),
        // "sqrt" => (
        //     |args| ops::unary_arithmetic_op(args, DynamicNumber::sqrt),
        //     FunctionArguments::unary(),
        // ),
        // "startswith" => (strings::startswith, FunctionArguments::binary()),
        // "strftime" => (temporal::strftime, FunctionArguments::binary()),
        "sub" => (ops::sub, FunctionArguments::variadic(2)),
        // "sum" => (ops::sum, FunctionArguments::unary()),
        // "s_stemmer" => (
        //     |args| ops::abstract_unary_string_fn(args, s_stemmer),
        //     FunctionArguments::unary(),
        // ),
        // "time" => (temporal::time, FunctionArguments::with_range(1..=2)),
        "to_fixed" => (fmt::to_fixed, FunctionArguments::binary()),
        // "to_timestamp" => (temporal::to_timestamp, FunctionArguments::unary()),
        // "to_timestamp_ms" => (temporal::to_timestamp_ms, FunctionArguments::unary()),
        // "to_timezone" | "to_tz" => (temporal::to_timezone, FunctionArguments::binary()),
        // "to_local_timezone" | "to_local_tz" => {
        //     (temporal::to_local_timezone, FunctionArguments::unary())
        // }
        "trim" => (fmt::trim, FunctionArguments::with_range(1..=2)),
        "ltrim" => (fmt::ltrim, FunctionArguments::with_range(1..=2)),
        "rtrim" => (fmt::rtrim, FunctionArguments::with_range(1..=2)),
        // "trunc" => (
        //     |args| ops::round_like_op(args, DynamicNumber::trunc),
        //     FunctionArguments::with_range(1..=2),
        // ),
        "typeof" => (utils::type_of, FunctionArguments::unary()),
        "unidecode" => (fuzzy::apply_unidecode, FunctionArguments::unary()),
        // "upper" => (strings::upper, FunctionArguments::unary()),
        // "urljoin" => (web::urljoin, FunctionArguments::binary()),
        "uuid" => (utils::uuid, FunctionArguments::nullary()),
        "values" => (maps::values, FunctionArguments::unary()),
        // "with_timezone" | "with_tz" => (temporal::with_timezone, FunctionArguments::binary()),
        // "with_local_timezone" | "with_local_tz" => {
        //     (temporal::with_local_timezone, FunctionArguments::unary())
        // }
        // "without_timezone" | "without_tz" => {
        //     (temporal::without_timezone, FunctionArguments::unary())
        // }
        // "write" => (io::write, FunctionArguments::binary()),
        // "year" => (
        //     |args| temporal::custom_strftime(args, "%Y"),
        //     FunctionArguments::unary(),
        // ),
        // "year_month_day" | "ymd" => (
        //     |args| temporal::custom_strftime(args, "%F"),
        //     FunctionArguments::unary(),
        // ),
        // "year_month" | "ym" => (
        //     |args| temporal::custom_strftime(args, "%Y-%m"),
        //     FunctionArguments::unary(),
        // ),
        _ => return None,
    })
}
