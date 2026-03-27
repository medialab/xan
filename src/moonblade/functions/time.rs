use jiff::tz::TimeZone;

use crate::dates::{parse_maybe_zoned, parse_maybe_zoned_with_format, MaybeZoned};

use super::FunctionResult;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

impl DynamicValue {
    fn try_into_maybe_zoned(self) -> Result<MaybeZoned, EvaluationError> {
        if let Self::Zoned(zoned) = self {
            return Ok(MaybeZoned::Zoned(*zoned));
        }

        if let Self::DateTime(datetime) = self {
            return Ok(MaybeZoned::Civil(datetime));
        }

        if self.is_temporal() {
            return Err(EvaluationError::from_cast(&self, "maybe_zoned"));
        }

        let bytes = self.try_as_bytes()?;

        match parse_maybe_zoned(bytes) {
            Err(_) => Err(EvaluationError::from_cast(&self, "maybe_zoned")),
            Ok(maybe) => Ok(maybe),
        }
    }

    // fn try_into_zoned(self) -> Result<Zoned, EvaluationError> {
    //     if let Self::Zoned(zoned) = self {
    //         return Ok(*zoned);
    //     }

    //     if self.is_temporal() {
    //         return Err(EvaluationError::from_cast(&self, "zoned"));
    //     }

    //     let bytes = self.try_as_bytes()?;

    //     match parse_maybe_zoned(bytes) {
    //         Err(_) => Err(EvaluationError::from_cast(&self, "zoned")),
    //         Ok(maybe) => match maybe {
    //             MaybeZoned::Civil(_) => Err(EvaluationError::from_cast(&self, "zoned")),
    //             MaybeZoned::Zoned(zoned) => Ok(zoned),
    //         },
    //     }
    // }

    // fn try_into_datetime(self) -> Result<DateTime, EvaluationError> {
    //     if let Self::DateTime(datetime) = self {
    //         return Ok(datetime);
    //     }

    //     if self.is_temporal() {
    //         return Err(EvaluationError::from_cast(&self, "datetime"));
    //     }

    //     let bytes = self.try_as_bytes()?;

    //     match parse_maybe_zoned(bytes) {
    //         Err(_) => Err(EvaluationError::from_cast(&self, "datetime")),
    //         Ok(maybe) => match maybe {
    //             MaybeZoned::Civil(datetime) => Ok(datetime),
    //             MaybeZoned::Zoned(_) => Err(EvaluationError::from_cast(&self, "datetime")),
    //         },
    //     }
    // }
}

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
            if matches!(arg, DynamicValue::Zoned(_) | DynamicValue::DateTime(_)) {
                return Err(EvaluationError::Custom(
                    "cannot parse a value that is already a datetime using a format".to_string(),
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

            // TODO: deal with other temporal types flowcharts

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

// TODO: add local_datetime?
// TODO: what to do with timestamps

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
