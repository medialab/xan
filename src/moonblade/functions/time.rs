use jiff::{
    civil::{Date, Time},
    tz::TimeZone,
};

use crate::dates::{
    parse_maybe_zoned, parse_maybe_zoned_with_format, MaybeZoned, DEFAULT_DATETIME_PARSER,
};

use super::FunctionResult;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

pub fn datetime(mut args: BoundArguments) -> FunctionResult {
    let (arg, format_arg_opt) = if args.len() == 2 {
        let (a, b) = args.pop2();
        (a, Some(b))
    } else {
        (args.pop1(), None)
    };

    match format_arg_opt {
        Some(format_arg) => {
            // Early returns mapping to errors
            if arg.is_temporal() {
                return Err(EvaluationError::Custom(
                    "cannot parse an already parsed temporal value using a format".to_string(),
                ));
            }

            // Using a strptime format
            let format = format_arg.try_as_bytes()?;
            let string = arg.try_as_bytes()?;

            parse_maybe_zoned_with_format(format, string)
                .map_err(|err| {
                    EvaluationError::TimeRelated(format!(
                        "{} (value: {:?}, format: {:?})",
                        err.as_str(),
                        arg,
                        format_arg
                    ))
                })
                .map(|maybe| match maybe {
                    MaybeZoned::Civil(datetime) => DynamicValue::from(datetime),
                    MaybeZoned::Zoned(zoned) => DynamicValue::from(zoned),
                })
        }
        None => {
            // Early returns mapping to no-ops
            if matches!(arg, DynamicValue::Zoned(_) | DynamicValue::DateTime(_)) {
                return Ok(arg);
            }

            // Attempting to parse
            let string = arg.try_as_bytes()?;

            match parse_maybe_zoned(string) {
                Err(err) => Err(EvaluationError::TimeRelated(format!(
                    "{} (value: {:?})",
                    err.as_str(),
                    arg,
                ))),
                Ok(maybe) => Ok(match maybe {
                    MaybeZoned::Civil(datetime) => DynamicValue::from(datetime),
                    MaybeZoned::Zoned(zoned) => DynamicValue::from(zoned),
                }),
            }
        }
    }
}

pub fn date(mut args: BoundArguments) -> FunctionResult {
    let (arg, format_arg_opt) = if args.len() == 2 {
        let (a, b) = args.pop2();
        (a, Some(b))
    } else {
        (args.pop1(), None)
    };

    match format_arg_opt {
        Some(format_arg) => {
            // Early returns mapping to errors
            if arg.is_temporal() {
                return Err(EvaluationError::Custom(
                    "cannot parse an already parsed temporal value using a format".to_string(),
                ));
            }

            // Using a strptime format
            let format = format_arg.try_as_bytes()?;
            let string = arg.try_as_bytes()?;

            match Date::strptime(format, string) {
                Err(_) => Err(EvaluationError::TimeRelated(format!(
                    "could not parse {:?} as a date using format {:?}",
                    arg, format_arg
                ))),
                Ok(date) => Ok(DynamicValue::from(date)),
            }
        }
        None => {
            // Early returns mapping to no-ops
            if matches!(arg, DynamicValue::Date(_)) {
                return Ok(arg);
            }

            match arg {
                DynamicValue::Zoned(zoned) => return Ok(DynamicValue::from(zoned.date())),
                DynamicValue::DateTime(datetime) => return Ok(DynamicValue::from(datetime.date())),
                _ => (),
            };

            // Attempting to parse
            let string = arg.try_as_bytes()?;

            match DEFAULT_DATETIME_PARSER.parse_date(string) {
                Err(_) => Err(EvaluationError::TimeRelated(format!(
                    "could not parse {:?} as a date",
                    arg
                ))),
                Ok(date) => Ok(DynamicValue::from(date)),
            }
        }
    }
}

pub fn time(mut args: BoundArguments) -> FunctionResult {
    let (arg, format_arg_opt) = if args.len() == 2 {
        let (a, b) = args.pop2();
        (a, Some(b))
    } else {
        (args.pop1(), None)
    };

    match format_arg_opt {
        Some(format_arg) => {
            // Early returns mapping to errors
            if arg.is_temporal() {
                return Err(EvaluationError::Custom(
                    "cannot parse an already parsed temporal value using a format".to_string(),
                ));
            }

            // Using a strptime format
            let format = format_arg.try_as_bytes()?;
            let string = arg.try_as_bytes()?;

            match Time::strptime(format, string) {
                Err(_) => Err(EvaluationError::TimeRelated(format!(
                    "could not parse {:?} as a time using format {:?}",
                    arg, format_arg
                ))),
                Ok(time) => Ok(DynamicValue::from(time)),
            }
        }
        None => {
            // Early returns mapping to no-ops
            if matches!(arg, DynamicValue::Time(_)) {
                return Ok(arg);
            }

            match arg {
                DynamicValue::Zoned(zoned) => return Ok(DynamicValue::from(zoned.time())),
                DynamicValue::DateTime(datetime) => return Ok(DynamicValue::from(datetime.time())),
                _ => (),
            };

            // Attempting to parse
            let string = arg.try_as_bytes()?;

            match DEFAULT_DATETIME_PARSER.parse_time(string) {
                Err(_) => Err(EvaluationError::TimeRelated(format!(
                    "could not parse {:?} as a time",
                    arg
                ))),
                Ok(time) => Ok(DynamicValue::from(time)),
            }
        }
    }
}

pub fn without_timezone(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    match arg.try_into_maybe_zoned() {
        Ok(MaybeZoned::Zoned(zoned)) => Ok(DynamicValue::from(zoned.datetime())),
        Ok(MaybeZoned::Civil(datetime)) => Err(EvaluationError::TimeRelated(format!(
            "can only remove its timezone to a datetime having one, but got {:?}",
            datetime
        ))),
        Err(err) => Err(err),
    }
}

pub fn with_timezone(mut args: BoundArguments) -> FunctionResult {
    let (arg, tz_arg) = args.pop2();

    match arg.try_into_maybe_zoned() {
        Ok(MaybeZoned::Zoned(zoned)) => Err(EvaluationError::TimeRelated(format!(
            "can only add a timezone to a datetime that does not have one already: {:?}. To convert a date to another timezone use `to_timezone` or `to_local_timezone` instead.",
            zoned
        ))),
        Ok(MaybeZoned::Civil(datetime)) => {
            let tz = tz_arg.try_as_timezone()?;

            datetime.to_zoned(tz).map_err(|_| EvaluationError::TimeRelated(
                "could not solve timezone ambiguity".to_string()
            )).map(DynamicValue::from)
        },
        Err(err) => Err(err),
    }
}

pub fn with_local_timezone(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    match arg.try_into_maybe_zoned() {
        Ok(MaybeZoned::Zoned(zoned)) => Err(EvaluationError::TimeRelated(format!(
            "can only add a timezone to a datetime that does not have one already: {:?}. To convert a date to another timezone use `to_timezone` or `to_local_timezone` instead.",
            zoned
        ))),
        Ok(MaybeZoned::Civil(datetime)) => {
            datetime.to_zoned(TimeZone::system()).map_err(|_| EvaluationError::TimeRelated(
                "could not solve timezone ambiguity".to_string()
            )).map(DynamicValue::from)
        },
        Err(err) => Err(err),
    }
}

pub fn to_timezone(mut args: BoundArguments) -> FunctionResult {
    let (arg, tz_arg) = args.pop2();

    match arg.try_into_maybe_zoned() {
        Ok(MaybeZoned::Zoned(zoned)) => {
            let tz = tz_arg.try_as_timezone()?;
            Ok(DynamicValue::from(zoned.with_time_zone(tz)))
        },
        Ok(MaybeZoned::Civil(_)) => Err(EvaluationError::TimeRelated(
            "cannot convert timezone of a datetime having no timezone. Use `with_timezone` or `with_local_timezone` to indicate its timezone instead.".to_string()
        )),
        Err(err) => Err(err)
    }
}

pub fn to_local_timezone(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    match arg.try_into_maybe_zoned() {
        Ok(MaybeZoned::Zoned(zoned)) => {
            Ok(DynamicValue::from(zoned.with_time_zone(TimeZone::system())))
        },
        Ok(MaybeZoned::Civil(_)) => Err(EvaluationError::TimeRelated(
            "cannot convert timezone of a datetime having no timezone. Use `with_timezone` or `with_local_timezone` to indicate its timezone instead.".to_string()
        )),
        Err(err) => Err(err)
    }
}

pub fn custom_strftime(mut args: BoundArguments, format: &str) -> FunctionResult {
    let arg = args.pop1();

    match arg.try_as_any_temporal()?.try_strftime(format) {
        Ok(string) => Ok(DynamicValue::from(string)),
        Err(reason) => Err(EvaluationError::TimeRelated(format!(
            "could not format {:?} using {:?} format. {}",
            arg, format, reason
        ))),
    }
}

pub fn strftime(mut args: BoundArguments) -> FunctionResult {
    let (arg, format_arg) = args.pop2();

    let format = format_arg.try_as_bytes()?;

    match arg.try_as_any_temporal()?.try_strftime(format) {
        Ok(string) => Ok(DynamicValue::from(string)),
        Err(reason) => Err(EvaluationError::TimeRelated(format!(
            "could not format {:?} using {:?} format. {}",
            arg, format_arg, reason
        ))),
    }
}
