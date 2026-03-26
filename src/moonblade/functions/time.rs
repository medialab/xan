use jiff::{
    civil::DateTime,
    fmt::{
        strtime,
        temporal::{DateTimeParser, PiecesOffset},
    },
    tz::TimeZone,
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

            let err = || -> Result<DynamicValue, EvaluationError> {
                Err(EvaluationError::TimeRelated(format!(
                    "cannot parse {:?} as a datetime using this format {:?}",
                    arg, format_arg
                )))
            };

            // Using a strptime format
            let format = format_arg.try_as_bytes()?;
            let string = arg.try_as_bytes()?;

            match strtime::parse(format, string) {
                Err(_) => err(),
                Ok(broken_down_time) => {
                    // If parsed time does not have any timezone info we attempt
                    // to parse it a simple datetime
                    if broken_down_time.offset().is_none()
                        && broken_down_time.iana_time_zone().is_none()
                    {
                        match broken_down_time.to_datetime() {
                            Err(_) => err(),
                            Ok(datetime) => Ok(DynamicValue::from(datetime)),
                        }
                    }
                    // Else we can attempt to parse it as a zoned
                    else {
                        match broken_down_time.to_zoned() {
                            Err(_) => err(),
                            Ok(zoned) => Ok(DynamicValue::from(zoned)),
                        }
                    }
                }
            }
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

                            // We have a zoned
                            match pieces.to_time_zone() {
                                Ok(Some(tz)) => match datetime.to_zoned(tz) {
                                    Err(_) => Err(EvaluationError::TimeRelated(format!(
                                        "{:?} is ambiguous",
                                        arg,
                                    ))),
                                    Ok(zoned) => Ok(DynamicValue::from(zoned)),
                                },
                                _ => Err(EvaluationError::TimeRelated(format!(
                                    "{:?} timezone information is invalid",
                                    arg,
                                ))),
                            }
                        }
                    }
                },
            }
        }
    }
}

// pub fn to_timezone(mut args: BoundArguments) -> FunctionResult {
//     let (datetime_arg, tz_arg) = args.pop2();

// }

// pub fn timestamp(args: BoundArguments) -> FunctionResult {
//     let seconds = args.get1().try_as_i64()?;
//     let utc = TimeZone::UTC;
//     match Timestamp::from_second(seconds) {
//         Ok(timestamp) => Ok(DynamicValue::from(timestamp.to_zoned(utc))),
//         Err(_) => Err(EvaluationError::TimeRelated(format!(
//             "cannot parse \"{}\" as timestamp",
//             seconds
//         ))),
//     }
// }

// pub fn timestamp_ms(args: BoundArguments) -> FunctionResult {
//     let milliseconds = args.get1().try_as_i64()?;
//     let utc = TimeZone::UTC;
//     match Timestamp::from_millisecond(milliseconds) {
//         Ok(timestamp) => Ok(DynamicValue::from(timestamp.to_zoned(utc))),
//         Err(_) => Err(EvaluationError::TimeRelated(format!(
//             "cannot parse \"{}\" as timestamp",
//             milliseconds
//         ))),
//     }
// }

// pub fn datetime(args: BoundArguments) -> FunctionResult {
//     let datestring = args.get1().try_as_str()?;
//     let format = args.get_not_none(1).map(|f| f.try_as_str()).transpose()?;
//     let timezone = args.get_not_none(2);

//     dates::parse_zoned(
//         &datestring,
//         format.as_deref(),
//         timezone.map(|tz| tz.try_as_timezone()).transpose()?,
//     )
//     .map_err(|err| {
//         EvaluationError::from_zoned_parse_error(
//             &datestring,
//             format.as_deref(),
//             timezone.map(|tz| tz.try_as_str().unwrap()).as_deref(),
//             err,
//         )
//     })
//     .map(DynamicValue::from)
// }

// pub fn to_timezone(args: BoundArguments) -> FunctionResult {
//     let (arg1, arg2, arg3) = args.get3();
//     // We could check if arg1 is a datetime before parsing it as str
//     let datestring = arg1.try_as_str()?;
//     let timezone_in = arg2.try_as_timezone()?;
//     let timezone_out = arg3.try_as_timezone()?;

//     dates::parse_zoned(&datestring, None, Some(timezone_in))
//         .map_err(|err| {
//             EvaluationError::from_zoned_parse_error(
//                 &datestring,
//                 None,
//                 Some(arg2.try_as_str().unwrap().as_ref()),
//                 err,
//             )
//         })
//         .map(|dt| DynamicValue::from(dt.with_time_zone(timezone_out)))
// }

// pub fn to_local_timezone(args: BoundArguments) -> FunctionResult {
//     let (arg1, arg2) = args.get2();
//     // We could check if arg1 is a datetime before parsing it as str
//     let datestring = arg1.try_as_str()?;
//     let timezone_in = arg2.try_as_timezone()?;

//     dates::parse_zoned(&datestring, None, Some(timezone_in))
//         .map_err(|err| {
//             EvaluationError::from_zoned_parse_error(
//                 &datestring,
//                 None,
//                 Some(arg2.try_as_str().unwrap().as_ref()),
//                 err,
//             )
//         })
//         .map(|dt| DynamicValue::from(dt.with_time_zone(TimeZone::system())))
// }

// fn abstract_strftime(datetime: &Zoned, format: &str) -> FunctionResult {
//     match strtime::format(format, datetime) {
//         Ok(formatted) => Ok(DynamicValue::from(formatted)),
//         Err(_) => Err(EvaluationError::TimeRelated(format!(
//             "\"{}\" is not a valid format",
//             format
//         ))),
//     }
// }

// pub fn strftime(args: BoundArguments) -> FunctionResult {
//     let (arg1, arg2) = args.get2();
//     let datetime = arg1.try_as_datetime()?;
//     let format = arg2.try_as_str()?;

//     abstract_strftime(&datetime, &format)
// }

// pub fn custom_strftime(args: BoundArguments, format: &str) -> FunctionResult {
//     let target = args.get1();
//     let datetime = target.try_as_datetime()?;

//     abstract_strftime(&datetime, format)
// }
