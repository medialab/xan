const LARGE_EPSILON: f64 = f64::EPSILON * 2.0;

// NOTE: this is not equivalent to casting since it will return None
// if the float is not near enough to a valid integer.
pub fn downgrade_float(f: f64) -> Option<i64> {
    let dust = f.fract();

    if dust + LARGE_EPSILON >= 1.0 || dust - LARGE_EPSILON <= 0.0 {
        Some(f.round() as i64)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downgrade_float() {
        assert_eq!(downgrade_float(0.0), Some(0i64));
        assert_eq!(downgrade_float(0.0), Some(0i64));
        assert_eq!(downgrade_float(0.5), None);
        assert_eq!(downgrade_float(f64::EPSILON), Some(0i64));
        assert_eq!(downgrade_float(-1.0 + f64::EPSILON), Some(-1i64));
        assert_eq!(downgrade_float(-1.0 - f64::EPSILON), Some(-1i64));
        assert_eq!(downgrade_float(1.0 + f64::EPSILON), Some(1i64));
        assert_eq!(downgrade_float(1.0 - f64::EPSILON), Some(1i64));
    }
}
