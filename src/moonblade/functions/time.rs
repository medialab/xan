use jiff::{
    civil::DateTime,
    fmt::{
        strtime,
        temporal::{DateTimeParser, PiecesOffset},
    },
    tz::{OffsetConflict, TimeZone},
};

use crate::dates::{
    parse_maybe_zoned, parse_maybe_zoned_with_format, MaybeZoned, MaybeZonedParseError,
};

use super::FunctionResult;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

static DEFAULT_DATETIME_PARSER: DateTimeParser = DateTimeParser::new();

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
                    "cannot call on a value that is already a datetime".to_string(),
                ));
            }

            // Using a strptime format
            let format = format_arg.try_as_bytes()?;
            let string = arg.try_as_bytes()?;

            parse_maybe_zoned_with_format(format, string)
                .map_err(|_| {
                    EvaluationError::TimeRelated(format!(
                        "cannot parse {:?} as a datetime using this format {:?}",
                        arg, format_arg
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

            match DEFAULT_DATETIME_PARSER.parse_pieces(string) {
                Err(_) => Err(EvaluationError::TimeRelated(format!(
                    "cannot parse {:?} as a datetime",
                    arg
                ))),
                Ok(pieces) => match pieces.time() {
                    None => Err(EvaluationError::TimeRelated(format!(
                        "{:?} does not contain a time",
                        arg,
                    ))),
                    Some(time) => {
                        let datetime = DateTime::from_parts(pieces.date(), time);

                        if pieces.offset().is_none() && pieces.time_zone_annotation().is_none() {
                            // We have a civil datetime
                            Ok(DynamicValue::from(datetime))
                        } else {
                            // We have a timestamp
                            if matches!(pieces.offset(), Some(PiecesOffset::Zulu)) {
                                return Ok(DynamicValue::from(
                                    datetime.to_zoned(TimeZone::UTC).unwrap(),
                                ));
                            }

                            let conflict_resolution = OffsetConflict::Reject;

                            // We might have a correct zoned
                            let ambiguous = match pieces.to_time_zone() {
                                Ok(None) => {
                                    let Some(offset) = pieces.to_numeric_offset() else {
                                        return Err(EvaluationError::TimeRelated(format!(
                                            "{:?} has no timezone nor offset",
                                            arg
                                        )));
                                    };

                                    TimeZone::fixed(offset).into_ambiguous_zoned(datetime)
                                }
                                Ok(Some(tz)) => match pieces.to_numeric_offset() {
                                    None => tz.into_ambiguous_zoned(datetime),
                                    Some(offset) => conflict_resolution
                                        .resolve(datetime, offset, tz)
                                        .map_err(|_| {
                                            EvaluationError::TimeRelated(format!(
                                                "{:?} has conflicting timezone & offset",
                                                arg
                                            ))
                                        })?,
                                },
                                Err(_) => {
                                    return Err(EvaluationError::TimeRelated(format!(
                                        "{:?} timezone information is invalid",
                                        arg,
                                    )));
                                }
                            };

                            match ambiguous.compatible() {
                                Err(_) => Err(EvaluationError::TimeRelated(format!(
                                        "{:?} datetime cannot be parsed because of unresolved unambiguity",
                                        arg,
                                    ))),
                                Ok(zoned) => Ok(DynamicValue::from(zoned))
                            }
                        }
                    }
                },
            }
        }
    }
}

// TODO: all functions below should be able to work by try_as
// TODO: add local_datetime
// TODO: what to do with timestamps

pub fn without_timezone(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    match arg {
        DynamicValue::Zoned(zoned) => Ok(DynamicValue::from(zoned.datetime())),
        _ => Err(EvaluationError::TimeRelated(format!(
            "can only call to remove timezone to a datetime having one, but got {:?}",
            arg
        ))),
    }
}

pub fn with_timezone(mut args: BoundArguments) -> FunctionResult {
    let (arg, tz_arg) = args.pop2();

    match arg {
        DynamicValue::DateTime(datetime) => {
            let tz = tz_arg.try_as_timezone()?;

            datetime.to_zoned(tz).map_err(|_| EvaluationError::TimeRelated(
                "could not solve ambiguity".to_string()
            )).map(DynamicValue::from)
        },
        _ => Err(EvaluationError::TimeRelated(format!(
            "can only add a timezone to a datetime that does not have one already {:?}. To convert a date to another timezone use `to_timezone` or `to_local_timezone` instead",
            arg
        ))),
    }
}

pub fn with_local_timezone(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    match arg {
        DynamicValue::DateTime(datetime) => {
            datetime.to_zoned(TimeZone::system()).map_err(|_| EvaluationError::TimeRelated(
                "could not solve ambiguity".to_string()
            )).map(DynamicValue::from)
        },
        _ => Err(EvaluationError::TimeRelated(format!(
            "can only add a timezone to a datetime that does not have one already {:?}. To convert a date to another timezone use `to_timezone` instead",
            arg
        ))),
    }
}

pub fn to_timezone(mut args: BoundArguments) -> FunctionResult {
    let (arg, tz_arg) = args.pop2();

    match arg {
        DynamicValue::Zoned(zoned) => {
            let tz = tz_arg.try_as_timezone()?;

            Ok(DynamicValue::from(zoned.with_time_zone(tz)))
        },
        DynamicValue::DateTime(_) => Err(EvaluationError::TimeRelated(
            "cannot convert timezone of a datetime having no timezone. Use `with_timezone` or `with_local_timezone` to indicate its timezone instead.".to_string()
        )),
        _ => Err(EvaluationError::TimeRelated(format!(
            "cannot convert timezone of a \"{}\"",
            arg.type_of()
        ))),
    }
}

pub fn to_local_timezone(mut args: BoundArguments) -> FunctionResult {
    let arg = args.pop1();

    match arg {
        DynamicValue::Zoned(zoned) => {
            Ok(DynamicValue::from(zoned.with_time_zone(TimeZone::system())))
        },
        DynamicValue::DateTime(_) => Err(EvaluationError::TimeRelated(
            "cannot convert timezone of a datetime having no timezone. Use `with_timezone` or `with_local_timezone` to indicate its timezone instead.".to_string()
        )),
        _ => Err(EvaluationError::TimeRelated(format!(
            "cannot convert timezone of a \"{}\"",
            arg.type_of()
        ))),
    }
}
