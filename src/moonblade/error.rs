use std::fmt::Display;
use std::ops::RangeInclusive;

use super::types::{Arity, ColumIndexationBy};

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
        Self::InvalidArity((
            name,
            InvalidArity {
                expected: Arity::Range(range),
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
pub struct SpecifiedBindingError {
    pub function_name: String,
    pub arg_index: Option<usize>,
    pub reason: BindingError,
}

impl Display for SpecifiedBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.arg_index {
            Some(i) => write!(
                f,
                "error when binding arg n°{} for \"{}\": {}",
                i + 1,
                self.function_name,
                self.reason
            ),
            None => write!(f, "error when binding expression: {}", self.reason),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum BindingError {
    ColumnOutOfRange(usize),
    UnicodeDecodeError,
    UnknownVariable(String),
}

impl Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ColumnOutOfRange(idx) => write!(f, "column \"{}\" is out of range", idx),
            Self::UnknownVariable(name) => write!(f, "unknown variable \"{}\"", name),
            Self::UnicodeDecodeError => write!(f, "unicode decode error"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct SpecifiedCallError {
    pub function_name: String,
    pub reason: CallError,
}

impl Display for SpecifiedCallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error when calling function \"{}\": {}",
            self.function_name, self.reason
        )
    }
}

#[derive(Debug, PartialEq)]
pub enum CallError {
    InvalidArity(InvalidArity),
    InvalidPath,
    NotImplemented(String),
    CannotOpenFile(String),
    CannotReadFile(String),
    Cast((String, String)),
    Custom(String),
    UnsupportedEncoding(String),
    UnsupportedDecoderTrap(String),
    DecodeError,
    ColumnNotFound(ColumIndexationBy),
}

impl CallError {
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
}

impl Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => write!(f, "invalid posix path"),
            Self::InvalidArity(arity) => arity.fmt(f),
            Self::CannotOpenFile(path) => {
                write!(f, "cannot open file {}", path)
            }
            Self::CannotReadFile(path) => write!(f, "cannot read file {}", path),
            Self::Custom(msg) => write!(f, "{}", msg),
            Self::Cast((from_type, to_type)) => write!(
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
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum EvaluationError {
    Binding(SpecifiedBindingError),
    Call(SpecifiedCallError),
}

impl Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(err) => err.fmt(f),
            Self::Call(err) => err.fmt(f),
        }
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub enum RunError {
    Prepare(ConcretizationError),
    Evaluation(EvaluationError),
}
