use btoi::btoi;
use jiff::{
    civil::{Date, DateTime, Time},
    fmt::strtime,
    fmt::temporal::{DateTimeParser, PiecesOffset},
    tz::{OffsetConflict, TimeZone},
    Error, SignedDuration, Timestamp, ToSpan, Unit, Zoned,
};

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

pub fn parse_partial_date(input: impl AsRef<[u8]>) -> Option<PartialDate> {
    let bytes = input.as_ref();

    match bytes.len() {
        4 => {
            if bytes[0] != b'1' && bytes[0] != b'2' {
                return None;
            }

            PartialDate::year(btoi::<i16>(bytes).ok()?)
        }
        7 => {
            if bytes[4] != b'-' {
                return None;
            }

            PartialDate::month(
                btoi::<i16>(&bytes[..4]).ok()?,
                btoi::<i8>(&bytes[5..]).ok()?,
            )
        }
        10 => {
            if bytes[4] != b'-' || bytes[7] != b'-' {
                return None;
            }

            PartialDate::day(
                btoi::<i16>(&bytes[..4]).ok()?,
                btoi::<i8>(&bytes[5..7]).ok()?,
                btoi::<i8>(&bytes[8..]).ok()?,
            )
        }
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

pub enum MaybeZoned {
    Civil(DateTime),
    Zoned(Zoned),
}

#[derive(Debug, PartialEq, PartialOrd)]
pub enum AnyTemporal {
    Zoned(Zoned),
    DateTime(DateTime),
    Date(Date),
    Time(Time),
}

impl AnyTemporal {
    pub fn try_strftime(&self, format: impl AsRef<[u8]>) -> Result<String, Error> {
        match self {
            Self::Zoned(zoned) => strtime::format(format, zoned),
            Self::DateTime(datetime) => strtime::format(format, *datetime),
            Self::Date(date) => strtime::format(format, *date),
            Self::Time(time) => strtime::format(format, *time),
        }
    }

    pub fn kind_as_str(&self) -> &'static str {
        match self {
            Self::Zoned(_) => "zoned",
            Self::DateTime(_) => "datetime",
            Self::Date(_) => "date",
            Self::Time(_) => "time",
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum FuzzyTemporal {
    Any(AnyTemporal),
    PartialDate(PartialDate),
    Timestamp(Timestamp),
}

#[derive(Debug)]
pub enum TemporalParseError {
    #[allow(dead_code)]
    CannotParse(Error),
    DoesNotContainTime,
    NoValidTimezoneInfo,
    ConflictingTimezoneAndOffset,
    UnresolvedAmbiguity,
}

impl TemporalParseError {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CannotParse(_) => "cannot parse as a datetime",
            Self::DoesNotContainTime => "does not contain a time",
            Self::NoValidTimezoneInfo => "does not contain valid timezone info",
            Self::ConflictingTimezoneAndOffset => "contains conflicting timezone and offset",
            Self::UnresolvedAmbiguity => "contains an unresolved ambiguity",
        }
    }
}

pub static DEFAULT_DATETIME_PARSER: DateTimeParser = DateTimeParser::new();

pub fn parse_maybe_zoned(input: impl AsRef<[u8]>) -> Result<MaybeZoned, TemporalParseError> {
    use TemporalParseError::*;

    match DEFAULT_DATETIME_PARSER.parse_pieces(&input) {
        Err(err) => Err(CannotParse(err)),
        Ok(pieces) => match pieces.time() {
            None => Err(DoesNotContainTime),
            Some(time) => {
                let datetime = DateTime::from_parts(pieces.date(), time);

                if pieces.offset().is_none() && pieces.time_zone_annotation().is_none() {
                    // We have a civil datetime
                    Ok(MaybeZoned::Civil(datetime))
                } else {
                    // We have a timestamp
                    if matches!(pieces.offset(), Some(PiecesOffset::Zulu)) {
                        return Ok(MaybeZoned::Zoned(
                            datetime.to_zoned(TimeZone::UTC).map_err(CannotParse)?,
                        ));
                    }

                    let conflict_resolution = OffsetConflict::Reject;

                    // We might have a correct zoned
                    let ambiguous = match pieces.to_time_zone() {
                        Ok(None) => {
                            let Some(offset) = pieces.to_numeric_offset() else {
                                return Err(NoValidTimezoneInfo);
                            };

                            TimeZone::fixed(offset).into_ambiguous_zoned(datetime)
                        }
                        Ok(Some(tz)) => match pieces.to_numeric_offset() {
                            None => tz.into_ambiguous_zoned(datetime),
                            Some(offset) => conflict_resolution
                                .resolve(datetime, offset, tz)
                                .map_err(|_| ConflictingTimezoneAndOffset)?,
                        },
                        Err(_) => {
                            return Err(NoValidTimezoneInfo);
                        }
                    };

                    match ambiguous.compatible() {
                        Err(_) => Err(UnresolvedAmbiguity),
                        Ok(zoned) => Ok(MaybeZoned::Zoned(zoned)),
                    }
                }
            }
        },
    }
}

pub fn parse_maybe_zoned_with_format(
    format: impl AsRef<[u8]>,
    input: impl AsRef<[u8]>,
) -> Result<MaybeZoned, TemporalParseError> {
    use TemporalParseError::*;

    match strtime::parse(format, input) {
        Err(err) => Err(CannotParse(err)),
        Ok(broken_down_time) => {
            // If parsed time does not have any timezone info we attempt
            // to parse it a simple datetime
            if broken_down_time.offset().is_none() && broken_down_time.iana_time_zone().is_none() {
                match broken_down_time.to_datetime() {
                    Err(err) => Err(CannotParse(err)),
                    Ok(datetime) => Ok(MaybeZoned::Civil(datetime)),
                }
            }
            // Else we can attempt to parse it as a zoned
            else {
                match broken_down_time.to_zoned() {
                    Err(err) => Err(CannotParse(err)),
                    Ok(zoned) => Ok(MaybeZoned::Zoned(zoned)),
                }
            }
        }
    }
}

pub fn parse_any_temporal(input: impl AsRef<[u8]>) -> Result<AnyTemporal, TemporalParseError> {
    use TemporalParseError::*;

    // Early exit matching a bare time
    if matches!(input.as_ref().get(2), Some(b':')) {
        return match DEFAULT_DATETIME_PARSER.parse_time(&input) {
            Err(err) => Err(CannotParse(err)),
            Ok(time) => Ok(AnyTemporal::Time(time)),
        };
    }

    match DEFAULT_DATETIME_PARSER.parse_pieces(&input) {
        Err(err) => Err(CannotParse(err)),
        Ok(pieces) => match pieces.time() {
            None => Ok(AnyTemporal::Date(pieces.date())),
            Some(time) => {
                let datetime = DateTime::from_parts(pieces.date(), time);

                if pieces.offset().is_none() && pieces.time_zone_annotation().is_none() {
                    // We have a civil datetime
                    Ok(AnyTemporal::DateTime(datetime))
                } else {
                    // We have a timestamp
                    if matches!(pieces.offset(), Some(PiecesOffset::Zulu)) {
                        return Ok(AnyTemporal::Zoned(
                            datetime.to_zoned(TimeZone::UTC).map_err(CannotParse)?,
                        ));
                    }

                    let conflict_resolution = OffsetConflict::Reject;

                    // We might have a correct zoned
                    let ambiguous = match pieces.to_time_zone() {
                        Ok(None) => {
                            let Some(offset) = pieces.to_numeric_offset() else {
                                return Err(NoValidTimezoneInfo);
                            };

                            TimeZone::fixed(offset).into_ambiguous_zoned(datetime)
                        }
                        Ok(Some(tz)) => match pieces.to_numeric_offset() {
                            None => tz.into_ambiguous_zoned(datetime),
                            Some(offset) => conflict_resolution
                                .resolve(datetime, offset, tz)
                                .map_err(|_| ConflictingTimezoneAndOffset)?,
                        },
                        Err(_) => {
                            return Err(NoValidTimezoneInfo);
                        }
                    };

                    match ambiguous.compatible() {
                        Err(_) => Err(UnresolvedAmbiguity),
                        Ok(zoned) => Ok(AnyTemporal::Zoned(zoned)),
                    }
                }
            }
        },
    }
}

// TODO: deal with ints/floats
pub fn parse_fuzzy_temporal(
    input: impl AsRef<[u8]>,
    parse_float: bool,
) -> Result<FuzzyTemporal, TemporalParseError> {
    use TemporalParseError::*;

    let bytes = input.as_ref();

    // Early exit matching a bare time
    if matches!(bytes.get(2), Some(b':')) {
        return match DEFAULT_DATETIME_PARSER.parse_time(&input) {
            Err(err) => Err(CannotParse(err)),
            Ok(time) => Ok(FuzzyTemporal::Any(AnyTemporal::Time(time))),
        };
    }

    // Early exit for float timestamp
    if parse_float {
        if let Ok(f) = fast_float::parse::<f64, &[u8]>(bytes) {
            let duration = SignedDuration::from_secs_f64(f);
            return Timestamp::from_duration(duration)
                .map_err(CannotParse)
                .map(FuzzyTemporal::Timestamp);
        }
    }

    // Early exit for year or month
    if bytes.len() == 4 || bytes.len() == 7 {
        if let Some(partial_date) = parse_partial_date(bytes) {
            return Ok(FuzzyTemporal::PartialDate(partial_date));
        }
    }

    match DEFAULT_DATETIME_PARSER.parse_pieces(&input) {
        Err(err) => Err(CannotParse(err)),
        Ok(pieces) => match pieces.time() {
            None => Ok(FuzzyTemporal::Any(AnyTemporal::Date(pieces.date()))),
            Some(time) => {
                let datetime = DateTime::from_parts(pieces.date(), time);

                if pieces.offset().is_none() && pieces.time_zone_annotation().is_none() {
                    // We have a civil datetime
                    Ok(FuzzyTemporal::Any(AnyTemporal::DateTime(datetime)))
                } else {
                    // We have a timestamp
                    if let Some(PiecesOffset::Zulu) = pieces.offset() {
                        return Ok(FuzzyTemporal::Timestamp(
                            PiecesOffset::Zulu
                                .to_numeric_offset()
                                .to_timestamp(datetime)
                                .map_err(CannotParse)?,
                        ));
                    }

                    let conflict_resolution = OffsetConflict::Reject;

                    // We might have a correct zoned
                    let ambiguous = match pieces.to_time_zone() {
                        Ok(None) => {
                            let Some(offset) = pieces.to_numeric_offset() else {
                                return Err(NoValidTimezoneInfo);
                            };

                            TimeZone::fixed(offset).into_ambiguous_zoned(datetime)
                        }
                        Ok(Some(tz)) => match pieces.to_numeric_offset() {
                            None => tz.into_ambiguous_zoned(datetime),
                            Some(offset) => conflict_resolution
                                .resolve(datetime, offset, tz)
                                .map_err(|_| ConflictingTimezoneAndOffset)?,
                        },
                        Err(_) => {
                            return Err(NoValidTimezoneInfo);
                        }
                    };

                    match ambiguous.compatible() {
                        Err(_) => Err(UnresolvedAmbiguity),
                        Ok(zoned) => Ok(FuzzyTemporal::Any(AnyTemporal::Zoned(zoned))),
                    }
                }
            }
        },
    }
}

pub fn looks_temporal(input: impl AsRef<[u8]>) -> bool {
    parse_fuzzy_temporal(input, false).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_partial_date() {
        let tests = [
            ("oucuoh", None),
            ("2023", PartialDate::year(2023)),
            ("1998-13", None),
            ("1998-10", PartialDate::month(1998, 10)),
            ("1998-10-34", None),
            ("1998-10-22", PartialDate::day(1998, 10, 22)),
            ("1998/10/22", None),
        ];

        for (string, expected) in tests {
            assert_eq!(parse_partial_date(string), expected, "{}", string);
        }
    }

    #[test]
    fn test_parse_fuzzy_temporal() {
        assert!(parse_fuzzy_temporal("-28800", false).is_err());
        assert_eq!(
            parse_fuzzy_temporal("-28800", true).unwrap(),
            FuzzyTemporal::Timestamp(Timestamp::from_second(-28800).unwrap())
        );
    }
}
