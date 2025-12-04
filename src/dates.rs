use jiff::{civil::Date, civil::DateTime, tz::TimeZone, Error, Timestamp, ToSpan, Unit, Zoned};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref PARTIAL_DATE_REGEX: Regex = Regex::new(r"^[12]\d{3}(?:-(?:0\d|1[012]))?$").unwrap();
}

#[derive(Copy, Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PartialDate {
    inner: Date,
    precision: Unit,
}

impl PartialDate {
    fn year(y: i16) -> Option<Self> {
        Some(Self {
            inner: Date::new(y, 1, 1).ok()?,
            precision: Unit::Year,
        })
    }

    fn month(y: i16, m: i8) -> Option<Self> {
        Some(Self {
            inner: Date::new(y, m, 1).ok()?,
            precision: Unit::Month,
        })
    }

    fn day(y: i16, m: i8, d: i8) -> Option<Self> {
        Some(Self {
            inner: Date::new(y, m, d).ok()?,
            precision: Unit::Day,
        })
    }

    pub fn into_inner(self) -> Date {
        self.inner
    }

    pub fn as_unit(&self) -> Unit {
        self.precision
    }

    pub fn as_date(&self) -> &Date {
        &self.inner
    }

    pub fn from_date(date: Date, unit: Unit) -> Self {
        Self {
            inner: date,
            precision: unit,
        }
    }

    pub fn next(&self) -> Self {
        Self {
            inner: next_partial_date(self.precision, &self.inner),
            precision: self.precision,
        }
    }

    pub fn previous(&self) -> Self {
        Self {
            inner: previous_partial_date(self.precision, &self.inner),
            precision: self.precision,
        }
    }
}

pub fn is_partial_date(string: &str) -> bool {
    PARTIAL_DATE_REGEX.is_match(string)
}

pub fn parse_partial_date(string: &str) -> Option<PartialDate> {
    match string.len() {
        4 => PartialDate::year(string.parse::<i16>().ok()?),
        7 => PartialDate::month(
            string[..4].parse::<i16>().ok()?,
            string[5..].parse::<i8>().ok()?,
        ),
        10 => PartialDate::day(
            string[..4].parse::<i16>().ok()?,
            string[5..7].parse::<i8>().ok()?,
            string[8..].parse::<i8>().ok()?,
        ),
        _ => None,
    }
}

pub fn next_partial_date(unit: Unit, date: &Date) -> Date {
    match unit {
        Unit::Year => date.checked_add(1.year()).unwrap(),
        Unit::Month => date.checked_add(1.month()).unwrap(),
        Unit::Day => date.checked_add(1.day()).unwrap(),
        _ => unimplemented!(),
    }
}

pub fn previous_partial_date(unit: Unit, date: &Date) -> Date {
    match unit {
        Unit::Year => date.checked_sub(1.year()).unwrap(),
        Unit::Month => date.checked_sub(1.month()).unwrap(),
        Unit::Day => date.checked_sub(1.day()).unwrap(),
        _ => unimplemented!(),
    }
}

pub fn format_partial_date(unit: Unit, date: &Date) -> String {
    match unit {
        Unit::Year => date.strftime("%Y").to_string(),
        Unit::Month => date.strftime("%Y-%m").to_string(),
        Unit::Day => date.strftime("%Y-%m-%d").to_string(),
        _ => unimplemented!(),
    }
}

const MINUTES_BOUND: i64 = 60;
const HOURS_BOUND: i64 = MINUTES_BOUND * 60;
const DAYS_BOUND: i64 = HOURS_BOUND * 24;
const MONTHS_BOUND: i64 = DAYS_BOUND * 30;
const YEARS_BOUND: i64 = MONTHS_BOUND * 12;

fn smallest_granularity(zoned: &Zoned) -> Unit {
    if zoned.month() == 1 {
        Unit::Year
    } else if zoned.day() == 1 {
        Unit::Month
    } else if zoned.hour() == 0 {
        Unit::Day
    } else if zoned.minute() == 0 {
        Unit::Hour
    } else if zoned.second() == 0 {
        Unit::Minute
    } else {
        Unit::Second
    }
}

pub fn infer_temporal_granularity(earliest: &Zoned, latest: &Zoned, graduations: usize) -> Unit {
    let duration = earliest.duration_until(latest);
    let seconds = duration.as_secs();

    let smallest = smallest_granularity(earliest).min(smallest_granularity(latest));

    let graduations = graduations as i64;

    let granularity = if seconds > YEARS_BOUND * graduations {
        Unit::Year
    } else if seconds > MONTHS_BOUND * graduations {
        Unit::Month
    } else if seconds > DAYS_BOUND * graduations {
        Unit::Day
    } else if seconds > HOURS_BOUND * graduations {
        Unit::Hour
    } else if seconds > MINUTES_BOUND * graduations {
        Unit::Minute
    } else {
        Unit::Second
    };

    granularity.max(smallest)
}

pub fn could_be_date(string: &str) -> bool {
    if string.ends_with('Z') {
        return string.parse::<Timestamp>().is_ok();
    }

    string.parse::<DateTime>().is_ok() || is_partial_date(string)
}

#[derive(Debug, Clone, Copy)]
enum JiffParseFailureKind {
    InvalidFormat,
    InvalidDate,
    NoTimezoneInfo,
    Unknown,
}

impl From<Error> for JiffParseFailureKind {
    fn from(value: Error) -> Self {
        let message = value.to_string();

        if message.contains("unrecognized directive") {
            return JiffParseFailureKind::InvalidFormat;
        }

        if message.contains("%Q") || message.contains("failed to find time zone") {
            return JiffParseFailureKind::NoTimezoneInfo;
        }

        if message.contains("parsing failed") {
            return JiffParseFailureKind::InvalidDate;
        }

        JiffParseFailureKind::Unknown
    }
}

impl JiffParseFailureKind {
    fn has_no_timezone_info(&self) -> bool {
        matches!(self, Self::NoTimezoneInfo)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ZonedParseError {
    CannotParse,
    InvalidFormat,
    TimezoneMismatch,
}

impl From<JiffParseFailureKind> for ZonedParseError {
    fn from(value: JiffParseFailureKind) -> Self {
        match value {
            JiffParseFailureKind::InvalidDate => Self::CannotParse,
            JiffParseFailureKind::InvalidFormat => Self::InvalidFormat,
            JiffParseFailureKind::Unknown => Self::CannotParse,
            _ => unreachable!(),
        }
    }
}

impl From<Error> for ZonedParseError {
    fn from(value: Error) -> Self {
        Self::from(JiffParseFailureKind::from(value))
    }
}

pub fn parse_zoned(
    value: &str,
    format: Option<&str>,
    timezone: Option<TimeZone>,
) -> Result<Zoned, ZonedParseError> {
    // With format
    if let Some(f) = format {
        return match Zoned::strptime(f, value) {
            Ok(zoned) => {
                // Matching the timezone
                if let Some(tz) = timezone.as_ref() {
                    if zoned.time_zone() != tz {
                        return Err(ZonedParseError::TimezoneMismatch);
                    }
                }

                Ok(zoned)
            }
            Err(err) => {
                let err_kind = JiffParseFailureKind::from(err);

                // We only attempt to parse as DateTime if no tzinfo was found by jiff
                if err_kind.has_no_timezone_info() {
                    match DateTime::strptime(f, value) {
                        Ok(datetime) => Ok(datetime
                            .to_zoned(timezone.unwrap_or_else(TimeZone::system))
                            .unwrap()),
                        Err(_) => Err(ZonedParseError::CannotParse),
                    }
                } else {
                    Err(ZonedParseError::from(err_kind))
                }
            }
        };
    }

    // Special case: timestamp formats
    if value.ends_with('Z') {
        if let Some(tz) = timezone.as_ref() {
            if tz != &TimeZone::UTC {
                return Err(ZonedParseError::TimezoneMismatch);
            }
        }

        match value.parse::<Timestamp>() {
            Ok(timestamp) => Ok(timestamp.to_zoned(TimeZone::UTC)),
            Err(err) => Err(ZonedParseError::from(err)),
        }
    } else {
        // Without format
        match value.parse::<Zoned>() {
            Ok(zoned) => {
                // Matching the timezone
                if let Some(tz) = timezone.as_ref() {
                    if zoned.time_zone() != tz {
                        return Err(ZonedParseError::TimezoneMismatch);
                    }
                }

                Ok(zoned)
            }
            Err(err) => {
                let err_kind = JiffParseFailureKind::from(err);

                // We only attempt to parse as DateTime if no tzinfo was found by jiff
                if err_kind.has_no_timezone_info() {
                    match value.parse::<DateTime>() {
                        Ok(datetime) => Ok(datetime
                            .to_zoned(timezone.unwrap_or_else(TimeZone::system))
                            .unwrap()),
                        Err(err) => Err(ZonedParseError::from(err)),
                    }
                } else {
                    Err(ZonedParseError::from(err_kind))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_partial_date() {
        let tests = [
            ("2023", true),
            ("999", false),
            ("3412", false),
            ("2023-01", true),
            ("999-45", false),
            ("2024-45", false),
            ("1999-01", true),
        ];

        for (string, expected) in tests {
            assert_eq!(is_partial_date(string), expected, "{}", string);
        }
    }

    #[test]
    fn test_parse_partial_date() {
        let tests = [
            ("oucuoh", None),
            ("2023", PartialDate::year(2023)),
            ("1998-13", None),
            ("1998-10", PartialDate::month(1998, 10)),
            ("1998-10-34", None),
            ("1998-10-22", PartialDate::day(1998, 10, 22)),
        ];

        for (string, expected) in tests {
            assert_eq!(parse_partial_date(string), expected, "{}", string);
        }
    }
}
