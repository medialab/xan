use std::fmt::Display;

use super::parser::ParseError;
use super::types::{Arity, ColumIndexationBy, DynamicValue};
use crate::dates::ZonedParseError;

fn format_column_indexation_error(
    f: &mut std::fmt::Formatter,
    indexation: &ColumIndexationBy,
) -> std::fmt::Result {
    match indexation {
        ColumIndexationBy::Name(name) => write!(f, "cannot find column \"{}\"", name),
        ColumIndexationBy::Pos(pos) => write!(f, "column {} out of range", pos),
        ColumIndexationBy::ReversePos(pos) => write!(f, "column {} out of range", -(*pos as isize)),
        ColumIndexationBy::NameAndNth((name, nth)) => {
            write!(f, "cannot find column (\"{}\", {})", name, nth)
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ConcretizationError {
    ParseError(ParseError),
    ColumnNotFound(ColumIndexationBy),
    InvalidRegex(String),
    UnknownFunction(String),
    InvalidArity(String, InvalidArity),
    TooManyArguments(usize),
    UnknownArgumentName(String),
    InvalidCSSSelector(String),
    StaticEvaluationError(SpecifiedEvaluationError),
    Custom(String),
    NotStaticallyAnalyzable,
}

impl Display for ConcretizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::ColumnNotFound(indexation) => format_column_indexation_error(f, indexation),
            Self::UnknownFunction(name) => write!(f, "unknown function \"{}\"", name),
            Self::UnknownArgumentName(arg_name) => write!(f, "unknown argument \"{}\"", arg_name),
            Self::ParseError(err) => write!(f, "could not parse expression: {}", err),
            Self::InvalidRegex(pattern) => write!(f, "invalid regex {}", pattern),
            Self::InvalidArity(name, arity) => write!(f, "{}: {}", name, arity),
            Self::TooManyArguments(actual) => {
                write!(f, "got {} arguments. Cannot exceed 8.", actual)
            }
            Self::InvalidCSSSelector(css) => write!(f, "invalid css selector: {}", css),
            Self::StaticEvaluationError(error) => error.fmt(f),
            Self::Custom(msg) => write!(f, "{}", msg),
            Self::NotStaticallyAnalyzable => write!(f, "not statically analyzable"),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct InvalidArity {
    expected: Arity,
    got: usize,
}

impl InvalidArity {
    pub fn from_arity(arity: Arity, got: usize) -> Self {
        Self {
            expected: arity,
            got,
        }
    }
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

impl SpecifiedEvaluationError {
    pub fn new(name: &str, reason: EvaluationError) -> Self {
        Self {
            function_name: name.to_string(),
            reason,
        }
    }
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
    InvalidLambda,
    NotImplemented(String),
    IO(String),
    DateTime(String),
    Cast(String, String),
    Custom(String),
    UnsupportedEncoding(String),
    UnsupportedDecoderTrap(String),
    DecodeError,
    ColumnNotFound(ColumIndexationBy),
    ColumnOutOfRange(usize),
    GlobalVariableOutOfRange(usize),
    UnicodeDecodeError,
    JSONParseError,
    UnfillableUnderscore,
}

impl EvaluationError {
    pub fn from_cast(from_value: &DynamicValue, expected: &str) -> Self {
        Self::Cast(from_value.type_of().to_string(), expected.to_string())
    }

    pub fn from_zoned_parse_error(
        value: &str,
        format: Option<&str>,
        timezone: Option<&str>,
        error: ZonedParseError,
    ) -> Self {
        Self::DateTime(match error {
            ZonedParseError::CannotParse => format!(
                "cannot parse \"{}\" as a datetime, consider using datetime() with a custom format",
                value
            ),
            ZonedParseError::TimezoneMismatch => format!(
                "conflicting timezones between \"{}\" and \"{}\"",
                value,
                timezone.unwrap()
            ),
            ZonedParseError::InvalidFormat => {
                format!("invalid strptime format: \"{}\"", format.unwrap())
            }
        })
    }

    pub fn specify(self, function_name: &str) -> SpecifiedEvaluationError {
        SpecifiedEvaluationError {
            function_name: function_name.to_string(),
            reason: self,
        }
    }

    pub fn anonymous(self) -> SpecifiedEvaluationError {
        SpecifiedEvaluationError {
            function_name: "<expr>".to_string(),
            reason: self,
        }
    }
}

impl Display for EvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => write!(f, "invalid posix path"),
            Self::InvalidArity(arity) => arity.fmt(f),
            Self::InvalidLambda => write!(f, "provided argument is not a lambda"),
            Self::IO(msg) => write!(f, "{}", msg),
            Self::DateTime(msg) => write!(f, "{}", msg),
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
            Self::ColumnOutOfRange(idx) => write!(f, "column {} is out of range", idx),
            Self::GlobalVariableOutOfRange(idx) => {
                write!(f, "global variable index={} is out of range", idx)
            }
            Self::UnicodeDecodeError => write!(f, "unicode decode error"),
            Self::JSONParseError => write!(f, "json parse error"),
            Self::UnfillableUnderscore => write!(
                f,
                "some underscore `_` was not fillable because it is not downstream of a pipe"
            ),
        }
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub enum RunError {
    Prepare(ConcretizationError),
    Evaluation(SpecifiedEvaluationError),
}
