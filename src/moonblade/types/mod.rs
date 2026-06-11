mod arguments;
mod bound_arguments;
mod dynamic_number;
mod dynamic_value;
mod headers;

pub use arguments::{Argument, Arity, FunctionArguments};
pub use bound_arguments::{
    BOUND_ARGUMENTS_CAPACITY, BoundArgument, BoundArguments, BoundContainer, BoundStringLike,
    LambdaArguments,
};
pub use dynamic_number::DynamicNumber;
pub use dynamic_value::DynamicValue;
pub use headers::{ColumIndexationBy, HeadersIndex};

use super::error::SpecifiedEvaluationError;

pub type EvaluationResult = Result<DynamicValue, SpecifiedEvaluationError>;
