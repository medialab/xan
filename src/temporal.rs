use btoi::btoi;
use jiff::{
    civil::{Date, DateTime, Time},
    fmt::strtime,
    fmt::temporal::{DateTimeParser, PiecesOffset},
    tz::{OffsetConflict, TimeZone},
    Error, SignedDuration, SpanRelativeTo, Timestamp, ToSpan, Unit, Zoned, ZonedRound,
};

#[derive(Clone, Deserialize)]
#[serde(try_from = "String")]
pub struct TimeZoneArg(TimeZone);

impl TimeZoneArg {
    pub fn into_inner(self) -> TimeZone {
        self.0
    }
}

impl TryFrom<String> for TimeZoneArg {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Ok(tz) = TimeZone::get(&value) {
            Ok(Self(tz))
        } else {
            Err(format!("unknown timezone \"{}\"", value))
        }
    }
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

pub enum MaybeZoned {
    Civil(DateTime),
    Zoned(Zoned),
}

#[derive(Debug, PartialEq, PartialOrd, Clone)]
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

    pub fn has_same_type(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Zoned(_), Self::Zoned(_))
                | (Self::DateTime(_), Self::DateTime(_))
                | (Self::Date(_), Self::Date(_))
                | (Self::Time(_), Self::Time(_))
        )
    }

    pub fn kind_as_str(&self) -> &'static str {
        match self {
            Self::Zoned(_) => "zoned",
            Self::DateTime(_) => "datetime",
            Self::Date(_) => "date",
            Self::Time(_) => "time",
        }
    }

    pub fn relative_total(&self, other: &Self, unit: Unit) -> Result<f64, Error> {
        match (self, other) {
            (AnyTemporal::Zoned(a), AnyTemporal::Zoned(b)) => {
                let total = b
                    .since(a)?
                    .total((unit, SpanRelativeTo::days_are_24_hours()))?;

                Ok(total)
            }

            (AnyTemporal::DateTime(a), AnyTemporal::DateTime(b)) => {
                let total = b
                    .since(*a)?
                    .total((unit, SpanRelativeTo::days_are_24_hours()))?;

                Ok(total)
            }

            (AnyTemporal::Date(a), AnyTemporal::Date(b)) => {
                let total = b
                    .since(*a)?
                    .total((unit, SpanRelativeTo::days_are_24_hours()))?;

                Ok(total)
            }

            (AnyTemporal::Time(a), AnyTemporal::Time(b)) => {
                let total = b
                    .since(*a)?
                    .total((unit, SpanRelativeTo::days_are_24_hours()))?;

                Ok(total)
            }

            _ => Err(Error::from_args(format_args!(
                "incompatible temporal types \"{}\" and \"{}\"",
                self.kind_as_str(),
                other.kind_as_str()
            ))),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum FuzzyTemporal {
    Any(AnyTemporal),
    PartialDate(PartialDate),
    Timestamp(Timestamp),
}

impl FuzzyTemporal {
    pub fn has_timezone(&self) -> bool {
        match self {
            Self::Any(temporal) => matches!(temporal, AnyTemporal::Zoned(_)),
            Self::PartialDate(_) => false,
            Self::Timestamp(_) => true,
        }
    }

    // NOTE: beware, this only converts timezone of Zoned elements as everything
    // else will be consider to be UTC!
    pub fn to_lower_bound_timestamp(&self, timezone: TimeZone) -> Result<Timestamp, Error> {
        Ok(match self {
            Self::Any(temporal) => match temporal {
                AnyTemporal::Zoned(zoned) => zoned.with_time_zone(timezone).timestamp(),
                AnyTemporal::DateTime(datetime) => datetime.to_zoned(TimeZone::UTC)?.timestamp(),
                AnyTemporal::Date(date) => date
                    .to_datetime(Time::default())
                    .to_zoned(TimeZone::UTC)?
                    .timestamp(),
                AnyTemporal::Time(_) => {
                    return Err(Error::from_args(format_args!(
                        "cannot convert a bare time to a lower bound timestamp"
                    )))
                }
            },
            Self::PartialDate(partial_date) => partial_date
                .as_date()
                .to_datetime(Time::default())
                .to_zoned(TimeZone::UTC)?
                .timestamp(),
            Self::Timestamp(timestamp) => *timestamp,
        })
    }
}

pub static DEFAULT_DATETIME_PARSER: DateTimeParser = DateTimeParser::new();

pub fn parse_maybe_zoned(input: impl AsRef<[u8]>) -> Result<MaybeZoned, Error> {
    let pieces = DEFAULT_DATETIME_PARSER.parse_pieces(&input)?;

    match pieces.time() {
        None => Ok(MaybeZoned::Civil(DateTime::from_parts(
            pieces.date(),
            Time::default(),
        ))),
        Some(time) => {
            let datetime = DateTime::from_parts(pieces.date(), time);

            if pieces.offset().is_none() && pieces.time_zone_annotation().is_none() {
                // We have a civil datetime
                Ok(MaybeZoned::Civil(datetime))
            } else {
                // We have a timestamp
                if matches!(pieces.offset(), Some(PiecesOffset::Zulu)) {
                    return Ok(MaybeZoned::Zoned(datetime.to_zoned(TimeZone::UTC)?));
                }

                let conflict_resolution = OffsetConflict::Reject;

                // We might have a correct zoned
                let ambiguous = match pieces.to_time_zone() {
                    Ok(None) => {
                        let Some(offset) = pieces.to_numeric_offset() else {
                            return Err(Error::from_args(format_args!("no valid timezone info")));
                        };

                        TimeZone::fixed(offset).into_ambiguous_zoned(datetime)
                    }
                    Ok(Some(tz)) => match pieces.to_numeric_offset() {
                        None => tz.into_ambiguous_zoned(datetime),
                        Some(offset) => conflict_resolution.resolve(datetime, offset, tz)?,
                    },
                    Err(_) => {
                        return Err(Error::from_args(format_args!("no valid timezone info")));
                    }
                };

                Ok(MaybeZoned::Zoned(ambiguous.compatible()?))
            }
        }
    }
}

pub fn parse_maybe_zoned_with_format(
    format: impl AsRef<[u8]>,
    input: impl AsRef<[u8]>,
) -> Result<MaybeZoned, Error> {
    let broken_down_time = strtime::parse(format, input)?;

    // If parsed time does not have any timezone info we attempt
    // to parse it a simple datetime
    if broken_down_time.offset().is_none() && broken_down_time.iana_time_zone().is_none() {
        Ok(MaybeZoned::Civil(broken_down_time.to_datetime()?))
    }
    // Else we can attempt to parse it as a zoned
    else {
        Ok(MaybeZoned::Zoned(broken_down_time.to_zoned()?))
    }
}

pub fn parse_any_temporal(input: impl AsRef<[u8]>) -> Result<AnyTemporal, Error> {
    // Early exit matching a bare time
    if matches!(input.as_ref().get(2), Some(b':')) {
        return Ok(AnyTemporal::Time(
            DEFAULT_DATETIME_PARSER.parse_time(&input)?,
        ));
    }

    let pieces = DEFAULT_DATETIME_PARSER.parse_pieces(&input)?;

    match pieces.time() {
        None => Ok(AnyTemporal::Date(pieces.date())),
        Some(time) => {
            let datetime = DateTime::from_parts(pieces.date(), time);

            if pieces.offset().is_none() && pieces.time_zone_annotation().is_none() {
                // We have a civil datetime
                Ok(AnyTemporal::DateTime(datetime))
            } else {
                // We have a timestamp
                if matches!(pieces.offset(), Some(PiecesOffset::Zulu)) {
                    return Ok(AnyTemporal::Zoned(datetime.to_zoned(TimeZone::UTC)?));
                }

                let conflict_resolution = OffsetConflict::Reject;

                // We might have a correct zoned
                let ambiguous = match pieces.to_time_zone() {
                    Ok(None) => {
                        let Some(offset) = pieces.to_numeric_offset() else {
                            return Err(Error::from_args(format_args!("no valid timezone info")));
                        };

                        TimeZone::fixed(offset).into_ambiguous_zoned(datetime)
                    }
                    Ok(Some(tz)) => match pieces.to_numeric_offset() {
                        None => tz.into_ambiguous_zoned(datetime),
                        Some(offset) => conflict_resolution.resolve(datetime, offset, tz)?,
                    },
                    Err(_) => {
                        return Err(Error::from_args(format_args!("no valid timezone info")));
                    }
                };

                Ok(AnyTemporal::Zoned(ambiguous.compatible()?))
            }
        }
    }
}

pub fn parse_fuzzy_temporal(
    input: impl AsRef<[u8]>,
    parse_float: bool,
) -> Result<FuzzyTemporal, Error> {
    let bytes = input.as_ref();

    // Early exit matching a bare time
    if matches!(bytes.get(2), Some(b':')) {
        return Ok(FuzzyTemporal::Any(AnyTemporal::Time(
            DEFAULT_DATETIME_PARSER.parse_time(&input)?,
        )));
    }

    // Early exit for float timestamp
    if parse_float {
        if let Ok(secs) = fast_float::parse::<f64, &[u8]>(bytes) {
            return Timestamp::from_secs_f64(secs).map(FuzzyTemporal::Timestamp);
        }
    }

    // Early exit for year or month
    if bytes.len() == 4 || bytes.len() == 7 {
        if let Some(partial_date) = parse_partial_date(bytes) {
            return Ok(FuzzyTemporal::PartialDate(partial_date));
        }
    }

    let pieces = DEFAULT_DATETIME_PARSER.parse_pieces(&input)?;

    match pieces.time() {
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
                            .to_timestamp(datetime)?,
                    ));
                }

                let conflict_resolution = OffsetConflict::Reject;

                // We might have a correct zoned
                let ambiguous = match pieces.to_time_zone() {
                    Ok(None) => {
                        let Some(offset) = pieces.to_numeric_offset() else {
                            return Err(Error::from_args(format_args!("no valid timezone info")));
                        };

                        TimeZone::fixed(offset).into_ambiguous_zoned(datetime)
                    }
                    Ok(Some(tz)) => match pieces.to_numeric_offset() {
                        None => tz.into_ambiguous_zoned(datetime),
                        Some(offset) => conflict_resolution.resolve(datetime, offset, tz)?,
                    },
                    Err(_) => {
                        return Err(Error::from_args(format_args!("no valid timezone info")));
                    }
                };

                Ok(FuzzyTemporal::Any(AnyTemporal::Zoned(
                    ambiguous.compatible()?,
                )))
            }
        }
    }
}

pub fn looks_temporal(input: impl AsRef<[u8]>) -> bool {
    parse_fuzzy_temporal(input, false).is_ok()
}

pub trait TimestampExt
where
    Self: Sized,
{
    fn from_secs_f64(secs: f64) -> Result<Self, Error>;
}

impl TimestampExt for Timestamp {
    fn from_secs_f64(secs: f64) -> Result<Self, Error> {
        let duration = SignedDuration::from_secs_f64(secs);
        Self::from_duration(duration)
    }
}

pub trait ZonedExt
where
    Self: Sized,
{
    fn floor(&self, unit: Unit) -> Result<Self, Error>;
}

impl ZonedExt for Zoned {
    fn floor(&self, unit: Unit) -> Result<Self, Error> {
        Ok(match unit {
            Unit::Year | Unit::Month => {
                if unit == Unit::Year {
                    self.start_of_day()?.first_of_year()?
                } else {
                    self.start_of_day()?.first_of_month()?
                }
            }
            _ => self.round(ZonedRound::new().smallest(unit))?,
        })
    }
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
