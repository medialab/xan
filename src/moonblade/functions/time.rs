use jiff::{fmt::strtime, tz::TimeZone, Timestamp, Zoned};

use crate::dates;

use super::FunctionResult;
use crate::moonblade::error::EvaluationError;
use crate::moonblade::types::{BoundArguments, DynamicValue};

pub fn timestamp(args: BoundArguments) -> FunctionResult {
    let seconds = args.get1().try_as_i64()?;
    let utc = TimeZone::UTC;
    match Timestamp::from_second(seconds) {
        Ok(timestamp) => Ok(DynamicValue::from(timestamp.to_zoned(utc))),
        Err(_) => Err(EvaluationError::DateTime(format!(
            "cannot parse \"{}\" as timestamp",
            seconds
        ))),
    }
}

pub fn timestamp_ms(args: BoundArguments) -> FunctionResult {
    let milliseconds = args.get1().try_as_i64()?;
    let utc = TimeZone::UTC;
    match Timestamp::from_millisecond(milliseconds) {
        Ok(timestamp) => Ok(DynamicValue::from(timestamp.to_zoned(utc))),
        Err(_) => Err(EvaluationError::DateTime(format!(
            "cannot parse \"{}\" as timestamp",
            milliseconds
        ))),
    }
}

pub fn datetime(args: BoundArguments) -> FunctionResult {
    let datestring = args.get1().try_as_str()?;
    let format = args.get_not_none(1).map(|f| f.try_as_str()).transpose()?;
    let timezone = args.get_not_none(2);

    dates::parse_zoned(
        &datestring,
        format.as_deref(),
        timezone.map(|tz| tz.try_as_timezone()).transpose()?,
    )
    .map_err(|err| {
        EvaluationError::from_zoned_parse_error(
            &datestring,
            format.as_deref(),
            timezone.map(|tz| tz.try_as_str().unwrap()).as_deref(),
            err,
        )
    })
    .map(DynamicValue::from)
}

pub fn to_timezone(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2, arg3) = args.get3();
    // We could check if arg1 is a datetime before parsing it as str
    let datestring = arg1.try_as_str()?;
    let timezone_in = arg2.try_as_timezone()?;
    let timezone_out = arg3.try_as_timezone()?;

    dates::parse_zoned(&datestring, None, Some(timezone_in))
        .map_err(|err| {
            EvaluationError::from_zoned_parse_error(
                &datestring,
                None,
                Some(arg2.try_as_str().unwrap().as_ref()),
                err,
            )
        })
        .map(|dt| DynamicValue::from(dt.with_time_zone(timezone_out)))
}

pub fn to_local_timezone(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();
    // We could check if arg1 is a datetime before parsing it as str
    let datestring = arg1.try_as_str()?;
    let timezone_in = arg2.try_as_timezone()?;

    dates::parse_zoned(&datestring, None, Some(timezone_in))
        .map_err(|err| {
            EvaluationError::from_zoned_parse_error(
                &datestring,
                None,
                Some(arg2.try_as_str().unwrap().as_ref()),
                err,
            )
        })
        .map(|dt| DynamicValue::from(dt.with_time_zone(TimeZone::system())))
}

fn abstract_strftime(datetime: &Zoned, format: &str) -> FunctionResult {
    match strtime::format(format, datetime) {
        Ok(formatted) => Ok(DynamicValue::from(formatted)),
        Err(_) => Err(EvaluationError::DateTime(format!(
            "\"{}\" is not a valid format",
            format
        ))),
    }
}

pub fn strftime(args: BoundArguments) -> FunctionResult {
    let (arg1, arg2) = args.get2();
    let datetime = arg1.try_as_datetime()?;
    let format = arg2.try_as_str()?;

    abstract_strftime(&datetime, &format)
}

pub fn custom_strftime(args: BoundArguments, format: &str) -> FunctionResult {
    let target = args.get1();
    let datetime = target.try_as_datetime()?;

    abstract_strftime(&datetime, format)
}
