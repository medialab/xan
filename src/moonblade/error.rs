use std::fmt::Display;
use std::ops::RangeInclusive;

use super::types::{Arity, ColumIndexationBy, DynamicValue};

fn format_column_indexation_error(
    f: &mut std::fmt::Formatter,
    indexation: &ColumIndexationBy,
) -> std::fmt::Result {
    match indexation {
        ColumIndexationBy::Name(name) => write!(f, "cannot find column \"{}\"", name),
        ColumIndexationBy::Pos(pos) => write!(f, "column {} out of range", pos),
        ColumIndexationBy::NameAndNth((name, nth)) => {
            write!(f, "cannot find column (\"{}\", {})", name, nth)
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ConcretizationError {
    ParseError(String),
    ColumnNotFound(ColumIndexationBy),
    InvalidRegex(String),
    UnknownFunction(String),
    InvalidArity((String, InvalidArity)),
    TooManyArguments(usize),
    NotStaticallyAnalyzable,
}

impl ConcretizationError {
    pub fn from_invalid_arity(name: String, expected: usize, got: usize) -> Self {
        Self::InvalidArity((
            name,
            InvalidArity {
                expected: Arity::Strict(expected),
                got,
            },
        ))
    }

    pub fn from_invalid_min_arity(name: String, min_expected: usize, got: usize) -> Self {
        Self::InvalidArity((
            name,
            InvalidArity {
                expected: Arity::Min(min_expected),
                got,
            },
        ))
    }

    pub fn from_invalid_range_arity(
        name: String,
        range: RangeInclusive<usize>,
        got: usize,
    ) -> Self {
        let count = range.clone().count();

        Self::InvalidArity((
            name,
            InvalidArity {
                expected: if count == 1 {
                    Arity::Strict(*range.start())
                } else {
                    Arity::Range(range)
                },
                got,
            },
        ))
    }
}

impl Display for ConcretizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ColumnNotFound(indexation) => format_column_indexation_error(f, indexation),
            Self::UnknownFunction(name) => write!(f, "unknown function \"{}\"", name),
            Self::ParseError(expr) => write!(f, "could not parse expression: {}", expr),
            Self::InvalidRegex(pattern) => write!(f, "invalid regex {}", pattern),
            Self::InvalidArity((name, arity)) => write!(f, "{}: {}", name, arity),
            Self::TooManyArguments(actual) => {
                write!(f, "got {} arguments. Cannot exceed 8.", actual)
            }
            Self::NotStaticallyAnalyzable => write!(f, "not statically analyzable"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct InvalidArity {
    expected: Arity,
    got: usize,
}

impl Display for InvalidArity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.expected {
            Arity::Min(min) => write!(
                f,
                "expected at least {} argument{} but got {}",
                min,
                if min > &1 { "s" } else { "" },
                self.got
            ),
            Arity::Strict(arity) => write!(
                f,
                "expected {} argument{} but got {}",
                arity,
                if arity > &1 { "s" } else { "" },
                self.got
            ),
            Arity::Range(range) => write!(
                f,
                "expected between {} and {} arguments but got {}",
                range.start(),
                range.end(),
                self.got
            ),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SpecifiedEvaluationError {
    pub function_name: String,
    pub reason: EvaluationError,
}

impl Display for SpecifiedEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error when calling function \"{}\": {}",
            self.function_name, self.reason
        )
    }
}

#[derive(Debug, PartialEq)]
pub enum EvaluationError {
    InvalidArity(InvalidArity),
    InvalidPath,
    NotImplemented(String),
    IO(String),
    Cast(String, String),
    Custom(String),
    UnsupportedEncoding(String),
    UnsupportedDecoderTrap(String),
    DecodeError,
    ColumnNotFound(ColumIndexationBy),
    ColumnOutOfRange(usize),
    UnicodeDecodeError,
    JSONParseError,
}

impl EvaluationError {
    pub fn from_invalid_arity(expected: usize, got: usize) -> Self {
        Self::InvalidArity(InvalidArity {
            expected: Arity::Strict(expected),
            got,
        })
    }

    pub fn from_invalid_min_arity(min_expected: usize, got: usize) -> Self {
        Self::InvalidArity(InvalidArity {
            expected: Arity::Min(min_expected),
            got,
        })
    }

    pub fn from_invalid_range_arity(range: RangeInclusive<usize>, got: usize) -> Self {
        Self::InvalidArity(InvalidArity {
            expected: Arity::Range(range),
            got,
        })
    }

    pub fn from_cast(from_value: &DynamicValue, expected: &str) -> Self {
        Self::Cast(from_value.type_of().to_string(), expected.to_string())
    }
}

impl Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => write!(f, "invalid posix path"),
            Self::InvalidArity(arity) => arity.fmt(f),
            Self::IO(msg) => write!(f, "{}", msg),
            Self::Custom(msg) => write!(f, "{}", msg),
            Self::Cast(from_type, to_type) => write!(
                f,
                "cannot safely cast from type \"{}\" to type \"{}\"",
                from_type, to_type
            ),
            Self::NotImplemented(t) => {
                write!(f, "not implemented for values of type \"{}\" as of yet", t)
            }
            Self::UnsupportedEncoding(name) => write!(f, "unsupported encoding \"{}\"", name),
            Self::UnsupportedDecoderTrap(name) => {
                write!(
                    f,
                    "unsupported encoder trap \"{}\". Must be one of strict, replace, ignore.",
                    name
                )
            }
            Self::DecodeError => write!(f, "could not decode"),
            Self::ColumnNotFound(indexation) => format_column_indexation_error(f, indexation),
            Self::ColumnOutOfRange(idx) => write!(f, "column \"{}\" is out of range", idx),
            Self::UnicodeDecodeError => write!(f, "unicode decode error"),
            Self::JSONParseError => write!(f, "json parse error"),
        }
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub enum RunError {
    Prepare(ConcretizationError),
    Evaluation(SpecifiedEvaluationError),
}
