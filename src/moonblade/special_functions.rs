// NOTE: the runtime function take a &[ConcreteExpr] instead of BoundArguments
// because they notoriously might want not to bind arguments in the first
// place (e.g. "if"/"unless").
use std::sync::Arc;

use csv::ByteRecord;

use super::error::{ConcretizationError, EvaluationError, SpecifiedEvaluationError};
use super::interpreter::{ConcreteExpr, EvaluationContext};
use super::parser::FunctionCall;
use super::types::{
    Arity, ColumIndexationBy, DynamicValue, EvaluationResult, FunctionArguments, LambdaArguments,
};

pub type ComptimeFunctionResult = Result<Option<ConcreteExpr>, ConcretizationError>;
pub type ComptimeFunction = fn(&FunctionCall, &ByteRecord) -> ComptimeFunctionResult;
pub type RuntimeFunction = fn(&EvaluationContext, &[ConcreteExpr]) -> EvaluationResult;

pub fn get_special_function(
    name: &str,
) -> Option<(
    Option<ComptimeFunction>,
    Option<RuntimeFunction>,
    FunctionArguments,
)> {
    macro_rules! higher_order_fn {
        ($name:expr, $variant:ident) => {
            (
                None,
                Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                    runtime_higher_order(context, args, $name, HigherOrderOperation::$variant)
                }),
                FunctionArguments::binary(),
            )
        };
    }

    Some(match name {
        // NOTE: col, cols and headers need a comptime version because static evaluation
        // is not an option for them. What's more they rely on the headers index which
        // is not available to normal functions.
        "col" => (
            Some(comptime_col),
            Some(runtime_col),
            FunctionArguments::with_range(1..=2),
        ),
        "col?" => (
            Some(comptime_unsure_col),
            Some(runtime_unsure_col),
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
                    ConcreteExpr::Value(DynamicValue::from(&headers[i]))
                })
            }),
            None,
            FunctionArguments::with_range(0..=2),
        ),
        // NOTE: index needs to be a special function because it relies on external
        // data that cannot be accessed by normal functions.
        "index" => (None, Some(runtime_index), FunctionArguments::nullary()),

        // NOTE: if and unless need to be special functions because they short-circuit
        // underlying evaluation and circumvent the typical DFS evaluation scheme.
        // NOTE: if and unless don't require a comptime version because static evaluation
        // will work just fine here.
        "if" => (None, Some(runtime_if), FunctionArguments::with_range(2..=3)),
        "unless" => (
            None,
            Some(runtime_unless),
            FunctionArguments::with_range(2..=3),
        ),

        // NOTE: try is special because you need to suppress the error if any
        "try" => (None, Some(runtime_try), FunctionArguments::unary()),

        // NOTE: lambda evaluation need to be a special function because, like
        // if and unless, they cannot work in DFS fashion unless you
        // bind some values ahead of time.
        // NOTE: higher-order functions work fine with static evaluation
        "map" => higher_order_fn!("map", Map),
        "filter" => higher_order_fn!("filter", Filter),

        _ => return None,
    })
}

fn comptime_col(call: &FunctionCall, headers: &ByteRecord) -> ComptimeFunctionResult {
    // Statically analyzable col() function call
    if let Some(column_indexation) = ColumIndexationBy::from_arguments(&call.raw_args_as_ref()) {
        match column_indexation.find_column_index(headers, headers.len()) {
            Some(index) => return Ok(Some(ConcreteExpr::Column(index))),
            None => return Err(ConcretizationError::ColumnNotFound(column_indexation)),
        };
    }

    Ok(None)
}

fn comptime_unsure_col(call: &FunctionCall, headers: &ByteRecord) -> ComptimeFunctionResult {
    // Statically analyzable col?() function call
    if let Some(column_indexation) = ColumIndexationBy::from_arguments(&call.raw_args_as_ref()) {
        match column_indexation.find_column_index(headers, headers.len()) {
            Some(index) => return Ok(Some(ConcreteExpr::Column(index))),
            None => return Ok(Some(ConcreteExpr::Value(DynamicValue::None))),
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
        Some(first_column_indexation) => match first_column_indexation
            .find_column_index(headers, headers.len())
        {
            Some(first_index) => {
                if call.args.len() < 2 {
                    Ok(Some(ConcreteExpr::List(
                        (first_index..headers.len()).map(map).collect(),
                    )))
                } else {
                    match ColumIndexationBy::from_argument(&call.args[1].1) {
                        None => Err(ConcretizationError::NotStaticallyAnalyzable),
                        Some(second_column_indexation) => {
                            match second_column_indexation.find_column_index(headers, headers.len())
                            {
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

fn runtime_if(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    let arity = args.len();

    let condition = &args[0];
    let result = condition.evaluate(context)?;

    let mut branch: Option<&ConcreteExpr> = None;

    if result.is_truthy() {
        branch = Some(&args[1]);
    } else if arity == 3 {
        branch = Some(&args[2]);
    }

    match branch {
        None => Ok(DynamicValue::None),
        Some(arg) => arg.evaluate(context),
    }
}

fn runtime_unless(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    let arity = args.len();

    let condition = &args[0];
    let result = condition.evaluate(context)?;

    let mut branch: Option<&ConcreteExpr> = None;

    if result.is_falsey() {
        branch = Some(&args[1]);
    } else if arity == 3 {
        branch = Some(&args[2]);
    }

    match branch {
        None => Ok(DynamicValue::None),
        Some(arg) => arg.evaluate(context),
    }
}

fn runtime_index(context: &EvaluationContext, _args: &[ConcreteExpr]) -> EvaluationResult {
    Ok(match context.index {
        None => DynamicValue::None,
        Some(index) => DynamicValue::from(index),
    })
}

fn runtime_col(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    let name_or_pos = args.first().unwrap().evaluate(context)?;

    let pos = match args.get(1) {
        Some(p) => Some(p.evaluate(context)?),
        None => None,
    };

    match ColumIndexationBy::from_bound_arguments(name_or_pos, pos) {
        None => Err(SpecifiedEvaluationError::new(
            "col",
            EvaluationError::Custom("invalid arguments".to_string()),
        )),
        Some(indexation) => match context.headers_index.get(&indexation) {
            None => Err(SpecifiedEvaluationError::new(
                "col",
                EvaluationError::ColumnNotFound(indexation),
            )),
            Some(index) => Ok(DynamicValue::from(&context.record[index])),
        },
    }
}

fn runtime_unsure_col(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    let name_or_pos = args.first().unwrap().evaluate(context)?;

    let pos = match args.get(1) {
        Some(p) => Some(p.evaluate(context)?),
        None => None,
    };

    match ColumIndexationBy::from_bound_arguments(name_or_pos, pos) {
        None => Err(SpecifiedEvaluationError::new(
            "col",
            EvaluationError::Custom("invalid arguments".to_string()),
        )),
        Some(indexation) => match context.headers_index.get(&indexation) {
            None => Ok(DynamicValue::None),
            Some(index) => Ok(DynamicValue::from(&context.record[index])),
        },
    }
}

fn runtime_try(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    let result = args.first().unwrap().evaluate(context);

    Ok(result.unwrap_or(DynamicValue::None))
}

#[derive(Clone, Copy)]
enum HigherOrderOperation {
    Filter,
    Map,
}

fn runtime_higher_order(
    context: &EvaluationContext,
    args: &[ConcreteExpr],
    name: &str,
    op: HigherOrderOperation,
) -> EvaluationResult {
    let list = args
        .first()
        .unwrap()
        .evaluate(context)?
        .try_into_arc_list()
        .map_err(|err| err.specify(name))?;

    let (names, lambda) = args
        .get(1)
        .unwrap()
        .try_as_lambda()
        .map_err(|err| err.anonymous())?;

    // Validating arity
    Arity::Strict(1)
        .validate(names.len())
        .map_err(|invalid_arity| EvaluationError::InvalidArity(invalid_arity).anonymous())?;

    let arg_name = names.first().unwrap();

    let mut variables = match context.lambda_variables {
        None => LambdaArguments::new(),
        Some(v) => v.clone(),
    };

    let item_arg_index = variables.register(arg_name);

    match op {
        HigherOrderOperation::Map => {
            let mut new_list = Vec::with_capacity(list.len());

            match Arc::try_unwrap(list) {
                Ok(owned_list) => {
                    for item in owned_list {
                        variables.set(item_arg_index, item);

                        let result = lambda.evaluate(&context.with_lambda_variables(&variables))?;
                        new_list.push(result);
                    }
                }
                Err(borrowed_list) => {
                    for item in borrowed_list.iter() {
                        variables.set(item_arg_index, item.clone());

                        let result = lambda.evaluate(&context.with_lambda_variables(&variables))?;
                        new_list.push(result);
                    }
                }
            }

            Ok(DynamicValue::from(new_list))
        }
        HigherOrderOperation::Filter => {
            let mut new_list = Vec::new();

            for item in list.iter() {
                variables.set(item_arg_index, item.clone());

                let result = lambda.evaluate(&context.with_lambda_variables(&variables))?;

                if result.is_truthy() {
                    new_list.push(item.clone());
                }
            }

            Ok(DynamicValue::from(new_list))
        }
    }
}
