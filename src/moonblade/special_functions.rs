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

#[derive(Debug, Clone, Copy)]
enum AbstractColReturnValue {
    Cell,
    Index,
    Header,
}

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
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_col(false, AbstractColReturnValue::Cell, call, headers)
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_col(false, AbstractColReturnValue::Cell, context, args)
            }),
            FunctionArguments::with_range(1..=2),
        ),
        "header" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_col(false, AbstractColReturnValue::Header, call, headers)
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_col(false, AbstractColReturnValue::Header, context, args)
            }),
            FunctionArguments::with_range(1..=2),
        ),
        "col_index" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_col(false, AbstractColReturnValue::Index, call, headers)
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_col(false, AbstractColReturnValue::Index, context, args)
            }),
            FunctionArguments::with_range(1..=2),
        ),
        "col?" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_col(true, AbstractColReturnValue::Cell, call, headers)
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_col(true, AbstractColReturnValue::Cell, context, args)
            }),
            FunctionArguments::with_range(1..=2),
        ),
        "header?" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_col(true, AbstractColReturnValue::Header, call, headers)
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_col(true, AbstractColReturnValue::Header, context, args)
            }),
            FunctionArguments::with_range(1..=2),
        ),
        "col_index?" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_col(true, AbstractColReturnValue::Index, call, headers)
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_col(true, AbstractColReturnValue::Index, context, args)
            }),
            FunctionArguments::with_range(1..=2),
        ),
        "cols" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_cols(call, headers, ConcreteExpr::Column)
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_cols(context, args, |i| DynamicValue::from(&context.record[i]))
            }),
            FunctionArguments::with_range(0..=2),
        ),
        "headers" => (
            Some(|call: &FunctionCall, headers: &ByteRecord| {
                abstract_comptime_cols(call, headers, |i| {
                    ConcreteExpr::Value(DynamicValue::from(&headers[i]))
                })
            }),
            Some(|context: &EvaluationContext, args: &[ConcreteExpr]| {
                abstract_runtime_cols(context, args, |i| {
                    DynamicValue::from(context.headers_index.get_at(i))
                })
            }),
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
        "and" => (None, Some(runtime_and), FunctionArguments::variadic(2)),
        "or" => (None, Some(runtime_or), FunctionArguments::variadic(2)),

        // NOTE: try is special because you need to suppress the error if any
        "try" => (None, Some(runtime_try), FunctionArguments::unary()),

        // NOTE: warn must know row index
        "warn" => (None, Some(runtime_warn), FunctionArguments::unary()),

        // NOTE: lambda evaluation need to be a special function because, like
        // if and unless, they cannot work in DFS fashion unless you
        // bind some values ahead of time.
        // NOTE: higher-order functions work fine with static evaluation
        "map" => higher_order_fn!("map", Map),
        "filter" => higher_order_fn!("filter", Filter),
        "find" => higher_order_fn!("find", Find),
        "find_index" => higher_order_fn!("find_index", FindIndex),

        _ => return None,
    })
}

fn abstract_comptime_col(
    unsure: bool,
    return_value: AbstractColReturnValue,
    call: &FunctionCall,
    headers: &ByteRecord,
) -> ComptimeFunctionResult {
    if let Some(column_indexation) = ColumIndexationBy::from_arguments(&call.raw_args_as_ref()) {
        match column_indexation.find_column_index(headers) {
            Some(index) => {
                return Ok(Some(match return_value {
                    AbstractColReturnValue::Cell => ConcreteExpr::Column(index),
                    AbstractColReturnValue::Index => ConcreteExpr::Value(DynamicValue::from(index)),
                    AbstractColReturnValue::Header => {
                        ConcreteExpr::Value(DynamicValue::from(&headers[index]))
                    }
                }))
            }
            None => {
                return if unsure {
                    Ok(Some(ConcreteExpr::Value(DynamicValue::None)))
                } else {
                    Err(ConcretizationError::ColumnNotFound(column_indexation))
                }
            }
        };
    }

    Ok(None)
}

fn abstract_comptime_cols<F>(
    call: &FunctionCall,
    headers: &ByteRecord,
    map: F,
) -> ComptimeFunctionResult
where
    F: Fn(usize) -> ConcreteExpr,
{
    if call.args.is_empty() {
        return Ok(Some(ConcreteExpr::List(
            (0..headers.len()).map(map).collect(),
        )));
    }

    match ColumIndexationBy::from_argument(&call.args[0].1) {
        None => Ok(None),
        Some(first_column_indexation) => match first_column_indexation.find_column_index(headers) {
            Some(first_index) => {
                if call.args.len() < 2 {
                    Ok(Some(ConcreteExpr::List(
                        (first_index..headers.len()).map(map).collect(),
                    )))
                } else {
                    match ColumIndexationBy::from_argument(&call.args[1].1) {
                        None => Ok(None),
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

fn runtime_or(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    debug_assert!(args.len() >= 2);

    let mut last: Option<DynamicValue> = None;

    for arg in args {
        let value = arg.evaluate(context)?;

        if value.is_truthy() {
            return Ok(value);
        }

        last.replace(value);
    }

    Ok(last.unwrap())
}

fn runtime_and(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    debug_assert!(args.len() >= 2);

    let mut last: Option<DynamicValue> = None;

    for arg in args {
        let value = arg.evaluate(context)?;

        if value.is_falsey() {
            return Ok(value);
        }

        last.replace(value);
    }

    Ok(last.unwrap())
}

fn runtime_index(context: &EvaluationContext, _args: &[ConcreteExpr]) -> EvaluationResult {
    Ok(match context.index {
        None => DynamicValue::None,
        Some(index) => DynamicValue::from(index),
    })
}

fn abstract_runtime_col(
    unsure: bool,
    return_value: AbstractColReturnValue,
    context: &EvaluationContext,
    args: &[ConcreteExpr],
) -> EvaluationResult {
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
            None => {
                if unsure {
                    Ok(DynamicValue::None)
                } else {
                    Err(SpecifiedEvaluationError::new(
                        "col",
                        EvaluationError::ColumnNotFound(indexation),
                    ))
                }
            }
            Some(index) => Ok(match return_value {
                AbstractColReturnValue::Index => DynamicValue::from(index),
                AbstractColReturnValue::Cell => DynamicValue::from(&context.record[index]),
                AbstractColReturnValue::Header => {
                    DynamicValue::from(context.headers_index.get_at(index))
                }
            }),
        },
    }
}

fn abstract_runtime_cols<F>(
    context: &EvaluationContext,
    args: &[ConcreteExpr],
    map: F,
) -> EvaluationResult
where
    F: Fn(usize) -> DynamicValue,
{
    // NOTE: 0 is not reachable because it can be resolved at comptime by definition
    match args.len() {
        1 => {
            let start_index_arg = args.first().unwrap().evaluate(context)?;

            match ColumIndexationBy::from_bound_arguments(start_index_arg, None) {
                None => Err(SpecifiedEvaluationError::new(
                    "col",
                    EvaluationError::Custom("invalid arguments".to_string()),
                )),
                Some(indexation) => match context.headers_index.get(&indexation) {
                    None => Err(SpecifiedEvaluationError::new(
                        "col",
                        EvaluationError::ColumnNotFound(indexation),
                    )),
                    Some(index) => Ok(DynamicValue::from(
                        (index..context.headers_index.len())
                            .map(map)
                            .collect::<Vec<_>>(),
                    )),
                },
            }
        }
        2 => {
            let start_index_arg = args.first().unwrap().evaluate(context)?;

            match ColumIndexationBy::from_bound_arguments(start_index_arg, None) {
                None => Err(SpecifiedEvaluationError::new(
                    "col",
                    EvaluationError::Custom("invalid arguments".to_string()),
                )),
                Some(start_indexation) => match context.headers_index.get(&start_indexation) {
                    None => Err(SpecifiedEvaluationError::new(
                        "col",
                        EvaluationError::ColumnNotFound(start_indexation),
                    )),
                    Some(start_index) => {
                        let end_index_arg = args.last().unwrap().evaluate(context)?;

                        match ColumIndexationBy::from_bound_arguments(end_index_arg, None) {
                            None => Err(SpecifiedEvaluationError::new(
                                "col",
                                EvaluationError::Custom("invalid arguments".to_string()),
                            )),
                            Some(end_indexation) => {
                                match context.headers_index.get(&end_indexation) {
                                    None => Err(SpecifiedEvaluationError::new(
                                        "col",
                                        EvaluationError::ColumnNotFound(end_indexation),
                                    )),
                                    Some(end_index) => {
                                        let range: Vec<_> = if start_index > end_index {
                                            (end_index..=start_index).map(map).rev().collect()
                                        } else {
                                            (start_index..=end_index).map(map).collect()
                                        };

                                        Ok(DynamicValue::from(range))
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
        _ => unreachable!(),
    }
}

fn runtime_try(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    let result = args.first().unwrap().evaluate(context);

    Ok(result.unwrap_or(DynamicValue::None))
}

fn runtime_warn(context: &EvaluationContext, args: &[ConcreteExpr]) -> EvaluationResult {
    let msg_arg = args.first().unwrap().evaluate(context)?;
    let msg = msg_arg.try_as_str().map_err(|err| err.specify("warn"))?;

    match context.index {
        Some(i) => eprintln!("Row index {}: {}", i, msg),
        None => eprintln!("{}", msg),
    };

    Ok(DynamicValue::None)
}

#[derive(Clone, Copy)]
enum HigherOrderOperation {
    Filter,
    Map,
    Find,
    FindIndex,
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
        HigherOrderOperation::Find => {
            for item in list.iter() {
                variables.set(item_arg_index, item.clone());

                let result = lambda.evaluate(&context.with_lambda_variables(&variables))?;

                if result.is_truthy() {
                    return Ok(item.clone());
                }
            }

            Ok(DynamicValue::None)
        }
        HigherOrderOperation::FindIndex => {
            for (i, item) in list.iter().enumerate() {
                variables.set(item_arg_index, item.clone());

                let result = lambda.evaluate(&context.with_lambda_variables(&variables))?;

                if result.is_truthy() {
                    return Ok(DynamicValue::from(i));
                }
            }

            Ok(DynamicValue::None)
        }
    }
}
