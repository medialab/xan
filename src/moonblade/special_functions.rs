// NOTE: the runtime function take a &[ConcreteExpr] instead of BoundArguments
// because they notoriously might want not to bind arguments in the first
// place (e.g. "if"/"unless").
use csv::ByteRecord;

use super::error::{ConcretizationError, EvaluationError, SpecifiedEvaluationError};
use super::interpreter::{ConcreteExpr, EvaluationContext, LambdaVariables};
use super::parser::FunctionCall;
use super::types::{ColumIndexationBy, DynamicValue, EvaluationResult, FunctionArguments};

pub type ComptimeFunctionResult = Result<Option<ConcreteExpr>, ConcretizationError>;
pub type ComptimeFunction = fn(&FunctionCall, &ByteRecord) -> ComptimeFunctionResult;
pub type RuntimeFunction = fn(
    Option<usize>,
    &ByteRecord,
    &EvaluationContext,
    Option<&LambdaVariables>,
    &[ConcreteExpr],
) -> EvaluationResult;

pub fn get_special_function(
    name: &str,
) -> Option<(
    Option<ComptimeFunction>,
    Option<RuntimeFunction>,
    FunctionArguments,
)> {
    Some(match name {
        "col" => (
            Some(comptime_col),
            Some(runtime_col),
            FunctionArguments::with_range(1..=2),
        ),
        "cols" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                comptime_cols_headers(call, headers, ConcreteExpr::Column)
            }),
            None,
            FunctionArguments::with_range(0..=2),
        ),
        "headers" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                comptime_cols_headers(call, headers, |i| {
                    ConcreteExpr::Value(DynamicValue::String(
                        std::str::from_utf8(&headers[i]).unwrap().to_string(),
                    ))
                })
            }),
            None,
            FunctionArguments::with_range(0..=2),
        ),
        "if" => (None, Some(runtime_if), FunctionArguments::with_range(2..=3)),
        "index" => (None, Some(runtime_index), FunctionArguments::nullary()),
        "unless" => (
            None,
            Some(runtime_unless),
            FunctionArguments::with_range(2..=3),
        ),
        _ => return None,
    })
}

fn comptime_col(call: &FunctionCall, headers: &ByteRecord) -> ComptimeFunctionResult {
    // Statically analyzable col() function call
    if let Some(column_indexation) = ColumIndexationBy::from_arguments(&call.raw_args_as_ref()) {
        match column_indexation.find_column_index(headers) {
            Some(index) => return Ok(Some(ConcreteExpr::Column(index))),
            None => return Err(ConcretizationError::ColumnNotFound(column_indexation)),
        };
    }

    Ok(None)
}

fn comptime_cols_headers<F>(
    call: &FunctionCall,
    headers: &ByteRecord,
    map: F,
) -> ComptimeFunctionResult
where
    F: Fn(usize) -> ConcreteExpr,
{
    match ColumIndexationBy::from_argument(&call.args[0].1) {
        None => Err(ConcretizationError::NotStaticallyAnalyzable),
        Some(first_column_indexation) => match first_column_indexation.find_column_index(headers) {
            Some(first_index) => {
                if call.args.len() < 2 {
                    Ok(Some(ConcreteExpr::List(
                        (first_index..headers.len()).map(map).collect(),
                    )))
                } else {
                    match ColumIndexationBy::from_argument(&call.args[1].1) {
                        None => Err(ConcretizationError::NotStaticallyAnalyzable),
                        Some(second_column_indexation) => {
                            match second_column_indexation.find_column_index(headers) {
                                Some(second_index) => {
                                    let range: Vec<_> = if first_index > second_index {
                                        (second_index..=first_index).map(map).rev().collect()
                                    } else {
                                        (first_index..=second_index).map(map).collect()
                                    };

                                    Ok(Some(ConcreteExpr::List(range)))
                                }
                                None => Err(ConcretizationError::ColumnNotFound(
                                    second_column_indexation,
                                )),
                            }
                        }
                    }
                }
            }
            None => Err(ConcretizationError::ColumnNotFound(first_column_indexation)),
        },
    }
}

fn runtime_if(
    index: Option<usize>,
    record: &ByteRecord,
    context: &EvaluationContext,
    lambda_variables: Option<&LambdaVariables>,
    args: &[ConcreteExpr],
) -> EvaluationResult {
    let arity = args.len();

    let condition = &args[0];
    let result = condition.evaluate(index, record, context, lambda_variables)?;

    let mut branch: Option<&ConcreteExpr> = None;

    if result.is_truthy() {
        branch = Some(&args[1]);
    } else if arity == 3 {
        branch = Some(&args[2]);
    }

    match branch {
        None => Ok(DynamicValue::None),
        Some(arg) => arg.evaluate(index, record, context, lambda_variables),
    }
}

fn runtime_unless(
    index: Option<usize>,
    record: &ByteRecord,
    context: &EvaluationContext,
    lambda_variables: Option<&LambdaVariables>,
    args: &[ConcreteExpr],
) -> EvaluationResult {
    let arity = args.len();

    let condition = &args[0];
    let result = condition.evaluate(index, record, context, lambda_variables)?;

    let mut branch: Option<&ConcreteExpr> = None;

    if result.is_falsey() {
        branch = Some(&args[1]);
    } else if arity == 3 {
        branch = Some(&args[2]);
    }

    match branch {
        None => Ok(DynamicValue::None),
        Some(arg) => arg.evaluate(index, record, context, lambda_variables),
    }
}

fn runtime_index(
    index: Option<usize>,
    _record: &ByteRecord,
    _context: &EvaluationContext,
    _lambda_variables: Option<&LambdaVariables>,
    _args: &[ConcreteExpr],
) -> EvaluationResult {
    Ok(match index {
        None => DynamicValue::None,
        Some(index) => DynamicValue::from(index),
    })
}

fn runtime_col(
    index: Option<usize>,
    record: &ByteRecord,
    context: &EvaluationContext,
    lambda_variables: Option<&LambdaVariables>,
    args: &[ConcreteExpr],
) -> EvaluationResult {
    let name_or_pos = args
        .first()
        .unwrap()
        .evaluate(index, record, context, lambda_variables)?;
    let pos = match args.get(1) {
        Some(p) => Some(p.evaluate(index, record, context, lambda_variables)?),
        None => None,
    };

    match ColumIndexationBy::from_bound_arguments(name_or_pos, pos) {
        None => Err(SpecifiedEvaluationError::new(
            "col",
            EvaluationError::Custom("invalid arguments".to_string()),
        )),
        Some(indexation) => match context.get_column_index(&indexation) {
            None => Err(SpecifiedEvaluationError::new(
                "col",
                EvaluationError::ColumnNotFound(indexation),
            )),
            Some(index) => match std::str::from_utf8(&record[index]) {
                Err(_) => Err(SpecifiedEvaluationError::new(
                    "col",
                    EvaluationError::UnicodeDecodeError,
                )),
                Ok(value) => Ok(DynamicValue::from(value)),
            },
        },
    }
}
