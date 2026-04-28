use std::fmt::Display;

use bstr::BStr;

use super::parser::ParseError;
use super::types::{Arity, ColumIndexationBy, DynamicValue};

fn format_column_indexation_error(
    f: &mut std::fmt::Formatter,
    indexation: &ColumIndexationBy,
    headless: bool,
) -> std::fmt::Result {
    match (indexation, headless) {
        (ColumIndexationBy::Name(name), false) => {
            write!(f, "cannot find column \"{}\"", BStr::new(name))
        }
        (ColumIndexationBy::Pos(pos), _) => write!(f, "column {} out of range", pos),
        (ColumIndexationBy::NameAndNth(name, nth), false) => {
            write!(f, "cannot find column (\"{}\", {})", BStr::new(name), nth)
        }
        (ColumIndexationBy::Name(name) | ColumIndexationBy::NameAndNth(name, _), true) => write!(
            f,
            "cannot find column \"{}\" by name in a file without header",
            BStr::new(name)
        ),
    }
}

#[derive(Debug, PartialEq)]
pub enum ConcretizationError {
    ParseError(ParseError),
    ColumnNotFound(ColumIndexationBy, bool),
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
            Self::ColumnNotFound(indexation, headless) => {
                format_column_indexation_error(f, indexation, *headless)
            }
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
            "{} {}",
            if self.function_name.starts_with('<') || self.function_name.starts_with('>') {
                self.function_name.clone()
            } else {
                format!("{}()", self.function_name)
            },
            self.reason
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
    TimeRelated(String),
    Cast {
        from_value: DynamicValue,
        to_type: String,
    },
    Custom(String),
    UnsupportedEncoding(String),
    UnsupportedDecoderTrap(String),
    ColumnNotFound(ColumIndexationBy, bool),
    ColumnOutOfRange(usize),
    GlobalVariableOutOfRange(usize),
    UnicodeDecodeError,
    JSONParseError(String),
    UnfillableUnderscore,
    PluralClauseMisalignment {
        got: usize,
        names: Vec<String>,
    },
}

impl EvaluationError {
    pub fn from_cell_cast(from_value: &[u8], expected: &str) -> Self {
        Self::Cast {
            from_value: DynamicValue::from(from_value),
            to_type: expected.to_string(),
        }
    }

    pub fn from_cast(from_value: &DynamicValue, expected: &str) -> Self {
        Self::Cast {
            from_value: from_value.clone(),
            to_type: expected.to_string(),
        }
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

    pub fn plural_clause_misalignment(names: &[String], got: usize) -> Self {
        Self::PluralClauseMisalignment {
            got,
            names: names.to_owned(),
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
            Self::TimeRelated(msg) => write!(f, "{}", msg),
            Self::Custom(msg) => write!(f, "{}", msg),
            Self::Cast {
                from_value,
                to_type,
            } => write!(
                f,
                "cannot safely cast {:?} from type \"{}\" to type \"{}\"",
                from_value,
                from_value.type_of(),
                to_type
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
            Self::ColumnNotFound(indexation, headless) => {
                format_column_indexation_error(f, indexation, *headless)
            }
            Self::ColumnOutOfRange(idx) => write!(f, "column {} is out of range", idx),
            Self::GlobalVariableOutOfRange(idx) => {
                write!(f, "global variable index={} is out of range", idx)
            }
            Self::UnicodeDecodeError => write!(f, "unicode decode error"),
            Self::JSONParseError(msg) => write!(f, "cannot parse {} as json", msg),
            Self::UnfillableUnderscore => write!(
                f,
                "some underscore `_` was not fillable because it is not downstream of a pipe"
            ),
            Self::PluralClauseMisalignment { got, names } => write!(
                f,
                "plural clause related to columns ({}) yielded {} items instead of {}",
                names.join(", "),
                got,
                names.len()
            ),
        }
    }
}

impl From<&str> for EvaluationError {
    fn from(value: &str) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<std::str::Utf8Error> for EvaluationError {
    fn from(_value: std::str::Utf8Error) -> Self {
        Self::UnicodeDecodeError
    }
}

impl From<jiff::Error> for EvaluationError {
    fn from(value: jiff::Error) -> Self {
        Self::TimeRelated(value.to_string())
    }
}

#[cfg(test)]
#[derive(Debug, PartialEq)]
pub enum RunError {
    Prepare(ConcretizationError),
    Evaluation(SpecifiedEvaluationError),
}
