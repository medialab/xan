use jiff::{civil::Date, ToSpan, Unit, Zoned};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref PARTIAL_DATE_REGEX: Regex = Regex::new(r"^[12]\d{3}(?:-(?:0\d|1[012]))?$").unwrap();
}

pub fn is_partial_date(string: &str) -> bool {
    PARTIAL_DATE_REGEX.is_match(string)
}

pub fn parse_partial_date(string: &str) -> Option<(Unit, Date)> {
    Some(match string.len() {
        4 => (
            Unit::Year,
            Date::new(string.parse::<i16>().ok()?, 1, 1).ok()?,
        ),
        7 => (
            Unit::Month,
            Date::new(
                string[..4].parse::<i16>().ok()?,
                string[5..].parse::<i8>().ok()?,
                1,
            )
            .ok()?,
        ),
        10 => (
            Unit::Day,
            Date::new(
                string[..4].parse::<i16>().ok()?,
                string[5..7].parse::<i8>().ok()?,
                string[8..].parse::<i8>().ok()?,
            )
            .ok()?,
        ),
        _ => return None,
    })
}

pub fn next_partial_date(unit: Unit, date: &Date) -> Date {
    match unit {
        Unit::Year => date.checked_add(1.year()).unwrap(),
        Unit::Month => date.checked_add(1.month()).unwrap(),
        Unit::Day => date.checked_add(1.day()).unwrap(),
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

pub fn infer_temporal_granularity(earliest: &Zoned, latest: &Zoned, graduations: usize) -> Unit {
    let duration = earliest.duration_until(latest);
    let seconds = duration.as_secs();

    let graduations = graduations as i64;

    if seconds > YEARS_BOUND * graduations {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

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
            ("2023", Some((Unit::Year, date(2023, 1, 1)))),
            ("1998-13", None),
            ("1998-10", Some((Unit::Month, date(1998, 10, 1)))),
            ("1998-10-34", None),
            ("1998-10-22", Some((Unit::Day, date(1998, 10, 22)))),
        ];

        for (string, expected) in tests {
            assert_eq!(parse_partial_date(string), expected, "{}", string);
        }
    }
}
