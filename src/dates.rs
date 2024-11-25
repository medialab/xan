use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref PARTIAL_DATE_REGEX: Regex = Regex::new(r"^[12]\d{3}(?:-(?:0\d|1[012]))?$").unwrap();
}

pub fn is_partial_date(string: &str) -> bool {
    PARTIAL_DATE_REGEX.is_match(string)
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
}
