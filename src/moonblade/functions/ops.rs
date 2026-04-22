use std::borrow::Cow;
use std::cmp::Ordering;
use std::ops::{Add, Sub};

use crate::temporal::AnyTemporal;

use crate::moonblade::agg::aggregators::{Sum, Welford};
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicNumber, DynamicValue};

use super::FunctionResult;

pub fn parse_int(args: BoundArguments) -> FunctionResult {
    args.get1().try_as_i64().map(DynamicValue::from)
}

pub fn parse_float(args: BoundArguments) -> FunctionResult {
    args.get1().try_as_f64().map(DynamicValue::from)
}

pub fn not(mut args: BoundArguments) -> FunctionResult {
    Ok(DynamicValue::from(!args.pop1_bool()))
}

pub fn arithmetic_op<F>(args: BoundArguments, op: F) -> FunctionResult
where
    F: FnOnce(DynamicNumber, DynamicNumber) -> DynamicNumber,
{
    let (a, b) = args.get2_number()?;
    Ok(DynamicValue::from(op(a, b)))
}

pub fn add(args: BoundArguments) -> FunctionResult {
    if args.len() == 2 {
        let (a, b) = args.get2();

        if let Some(span) = b.as_span() {
            return match a.try_as_any_temporal()? {
                AnyTemporal::Zoned(zoned) => match zoned.checked_add(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(zoned) => Ok(DynamicValue::from(zoned)),
                },
                AnyTemporal::DateTime(datetime) => match datetime.checked_add(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(datetime) => Ok(DynamicValue::from(datetime)),
                },
                AnyTemporal::Date(date) => match date.checked_add(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(date) => Ok(DynamicValue::from(date)),
                },
                AnyTemporal::Time(time) => match time.checked_add(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(time) => Ok(DynamicValue::from(time)),
                },
            };
        }
    }

    variadic_arithmetic_op(args, Add::add)
}

pub fn sub(args: BoundArguments) -> FunctionResult {
    if args.len() == 2 {
        let (a, b) = args.get2();

        if let Some(span) = b.as_span() {
            return match a.try_as_any_temporal()? {
                AnyTemporal::Zoned(zoned) => match zoned.checked_sub(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(zoned) => Ok(DynamicValue::from(zoned)),
                },
                AnyTemporal::DateTime(datetime) => match datetime.checked_sub(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(datetime) => Ok(DynamicValue::from(datetime)),
                },
                AnyTemporal::Date(date) => match date.checked_sub(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(date) => Ok(DynamicValue::from(date)),
                },
                AnyTemporal::Time(time) => match time.checked_sub(span) {
                    Err(err) => Err(EvaluationError::TimeRelated(err.to_string())),
                    Ok(time) => Ok(DynamicValue::from(time)),
                },
            };
        }
    }

    variadic_arithmetic_op(args, Sub::sub)
}

pub fn abstract_compare<F>(mut args: BoundArguments, validate: F) -> FunctionResult
where
    F: FnOnce(Ordering) -> bool,
{
    let (a, b) = args.pop2();

    let ordering = if let Some(a) = a.as_any_temporal() {
        let b = b.try_as_any_temporal()?;
        Some(a.try_cmp(&b)?)
    } else if let Some(b) = b.as_any_temporal() {
        let a = a.try_as_any_temporal()?;
        Some(a.try_cmp(&b)?)
    } else {
        a.try_as_number()?.partial_cmp(&b.try_as_number()?)
    };

    Ok(DynamicValue::from(match ordering {
        Some(ordering) => validate(ordering),
        None => false,
    }))
}

pub fn sequence_compare<F>(args: BoundArguments, validate: F) -> FunctionResult
where
    F: FnOnce(Ordering) -> bool,
{
    let (a, b) = args.get2();

    let ordering = if let (Some(a), Some(b)) = (a.as_bytes(), b.as_bytes()) {
        a.cmp(b)
    } else {
        a.try_as_str()?.cmp(&b.try_as_str()?)
    };

    // TODO: deal with lists
    Ok(validate(ordering).into())
}

pub fn abstract_unary_string_fn<F>(args: BoundArguments, function: F) -> FunctionResult
where
    F: FnOnce(&str) -> Cow<str>,
{
    let string = args.get1().try_as_str()?;

    Ok(DynamicValue::from(function(&string)))
}

pub fn mean(args: BoundArguments) -> FunctionResult {
    let items = args.get1().try_as_list()?;
    let mut welford = Welford::new();

    for item in items {
        let n = item.try_as_f64()?;
        welford.add(n);
    }

    Ok(DynamicValue::from(welford.mean()))
}

pub fn sum(args: BoundArguments) -> FunctionResult {
    let items = args.get1().try_as_list()?;
    let mut sum = Sum::new();

    for item in items {
        sum.add(item.try_as_number()?);
    }

    Ok(DynamicValue::from(sum.get()))
}

pub fn variadic_arithmetic_op<F>(args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber, DynamicNumber) -> DynamicNumber,
{
    if args.len() == 2 {
        let (x, y) = args.get2_number()?;

        return Ok(op(x, y).into());
    }

    let mut args_iter = args.into_iter();

    let mut acc = args_iter.next().unwrap().try_as_number()?;

    for arg in args_iter {
        let cur = arg.try_as_number()?;
        acc = op(acc, cur);
    }

    Ok(DynamicValue::from(acc))
}

pub fn unary_arithmetic_op<F>(mut args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber) -> DynamicNumber,
{
    Ok(DynamicValue::from(op(args.pop1_number()?)))
}

pub fn round_like_op<F>(mut args: BoundArguments, op: F) -> FunctionResult
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

pub fn binary_arithmetic_op<F>(args: BoundArguments, op: F) -> FunctionResult
where
    F: Fn(DynamicNumber, DynamicNumber) -> DynamicNumber,
{
    let (n1, n2) = args.get2_number()?;

    Ok(DynamicValue::from(op(n1, n2)))
}

pub fn variadic_optimum<F, V, T>(args: BoundArguments, convert: F, validate: V) -> FunctionResult
where
    F: Fn(&DynamicValue) -> Result<T, EvaluationError>,
    V: Fn(Ordering) -> bool,
    T: PartialOrd,
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

            match other.partial_cmp(&best_value) {
                None => {
                    return Err(EvaluationError::Custom(
                        "trying to compare heterogenous types".to_string(),
                    ));
                }
                Some(ordering) => {
                    if validate(ordering) {
                        best_value = other;
                    }
                }
            }
        }

        return Ok(DynamicValue::from(best_value));
    }

    let mut args_iter = args.into_iter();
    let mut best_value = convert(&args_iter.next().unwrap().to_value())?;

    for arg in args_iter {
        let other_value = convert(&arg.to_value())?;

        match other_value.partial_cmp(&best_value) {
            None => {
                return Err(EvaluationError::Custom(
                    "trying to compare heterogenous types".to_string(),
                ));
            }
            Some(ordering) => {
                if validate(ordering) {
                    best_value = other_value;
                }
            }
        }
    }

    Ok(DynamicValue::from(best_value))
}

pub fn argcompare<F>(args: BoundArguments, validate: F) -> FunctionResult
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
