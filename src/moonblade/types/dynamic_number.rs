use std::cmp::{Ord, Ordering, PartialOrd};
use std::convert::TryFrom;
use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Rem, Sub};
use std::str::FromStr;

use btoi::btoi;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
pub enum DynamicNumber {
    Float(f64),
    Integer(i64),
}

impl DynamicNumber {
    pub fn abs(self) -> Self {
        match self {
            Self::Float(n) => Self::Float(n.abs()),
            Self::Integer(n) => Self::Integer(n.abs()),
        }
    }

    pub fn as_float(self) -> f64 {
        match self {
            Self::Float(f) => f,
            Self::Integer(i) => i as f64,
        }
    }

    pub fn as_int(self) -> i64 {
        match self {
            Self::Float(f) => f as i64,
            Self::Integer(i) => i,
        }
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    pub fn idiv(self, rhs: Self) -> Self {
        Self::Integer(match self {
            Self::Integer(a) => match rhs {
                Self::Integer(b) => return Self::Integer(a / b),
                Self::Float(b) => (a as f64).div_euclid(b) as i64,
            },
            Self::Float(a) => match rhs {
                Self::Integer(b) => a.div_euclid(b as f64) as i64,
                Self::Float(b) => a.div_euclid(b) as i64,
            },
        })
    }

    pub fn pow(self, rhs: Self) -> Self {
        match rhs {
            Self::Integer(e) => match self {
                Self::Integer(n) => {
                    if e >= 0 && e <= u32::MAX as i64 {
                        DynamicNumber::Integer(n.pow(e as u32))
                    } else {
                        DynamicNumber::Float((n as f64).powf(e as f64))
                    }
                }
                Self::Float(n) => {
                    if e >= i32::MIN as i64 && e <= i32::MAX as i64 {
                        DynamicNumber::Float(n.powi(e as i32))
                    } else {
                        DynamicNumber::Float(n.powf(e as f64))
                    }
                }
            },
            Self::Float(e) => match self {
                DynamicNumber::Integer(n) => DynamicNumber::Float((n as f64).powf(e)),
                DynamicNumber::Float(n) => DynamicNumber::Float(n.powf(e)),
            },
        }
    }

    pub fn map_float<F>(self, callback: F) -> Self
    where
        F: Fn(f64) -> f64,
    {
        match self {
            Self::Integer(a) => Self::Float(callback(a as f64)),
            Self::Float(a) => Self::Float(callback(a)),
        }
    }

    pub fn map_float_to_int<F>(self, callback: F) -> Self
    where
        F: Fn(f64) -> f64,
    {
        match self {
            Self::Integer(_) => self,
            Self::Float(n) => Self::Integer(callback(n) as i64),
        }
    }

    pub fn floor(self) -> Self {
        self.map_float_to_int(f64::floor)
    }

    pub fn ceil(self) -> Self {
        self.map_float_to_int(f64::ceil)
    }

    pub fn trunc(self) -> Self {
        self.map_float_to_int(f64::trunc)
    }

    pub fn round(self) -> Self {
        self.map_float_to_int(f64::round)
    }

    pub fn ln(self) -> Self {
        self.map_float(f64::ln)
    }

    pub fn exp(self) -> Self {
        self.map_float(f64::exp)
    }

    pub fn sqrt(self) -> Self {
        self.map_float(f64::sqrt)
    }

    pub fn is_nan(&self) -> bool {
        match self {
            Self::Integer(_) => false,
            Self::Float(f) => f.is_nan(),
        }
    }
}

impl TryFrom<String> for DynamicNumber {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value
            .parse::<Self>()
            .map_err(|_| format!("cannot parse {} as number", &value))
    }
}

impl fmt::Display for DynamicNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(n) => n.fmt(f),
            Self::Float(n) => n.fmt(f),
        }
    }
}

impl PartialEq for DynamicNumber {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Float(self_value) => match other {
                Self::Float(other_value) => self_value == other_value,
                Self::Integer(other_value) => *self_value == (*other_value as f64),
            },
            Self::Integer(self_value) => match other {
                Self::Float(other_value) => (*self_value as f64) == *other_value,
                Self::Integer(other_value) => self_value == other_value,
            },
        }
    }
}

impl Eq for DynamicNumber {}

impl PartialOrd for DynamicNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DynamicNumber {
    // TODO: NaN is gonna bite us in the buttocks at one point I'm sure..
    fn cmp(&self, other: &Self) -> Ordering {
        (match self {
            Self::Float(self_value) => match other {
                Self::Float(other_value) => self_value.partial_cmp(other_value),
                Self::Integer(other_value) => self_value.partial_cmp(&(*other_value as f64)),
            },
            Self::Integer(self_value) => match other {
                Self::Float(other_value) => (*self_value as f64).partial_cmp(other_value),
                Self::Integer(other_value) => Some(self_value.cmp(other_value)),
            },
        })
        .unwrap()
    }
}

fn apply_op<F1, F2>(
    lhs: DynamicNumber,
    rhs: DynamicNumber,
    op_int: F1,
    op_float: F2,
) -> DynamicNumber
where
    F1: FnOnce(i64, i64) -> i64,
    F2: FnOnce(f64, f64) -> f64,
{
    match lhs {
        DynamicNumber::Integer(a) => match rhs {
            DynamicNumber::Integer(b) => DynamicNumber::Integer(op_int(a, b)),
            DynamicNumber::Float(b) => DynamicNumber::Float(op_float(a as f64, b)),
        },
        DynamicNumber::Float(a) => match rhs {
            DynamicNumber::Integer(b) => DynamicNumber::Float(op_float(a, b as f64)),
            DynamicNumber::Float(b) => DynamicNumber::Float(op_float(a, b)),
        },
    }
}

impl Neg for DynamicNumber {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Float(v) => DynamicNumber::Float(-v),
            Self::Integer(v) => DynamicNumber::Integer(-v),
        }
    }
}

impl Rem for DynamicNumber {
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Rem::<i64>::rem, Rem::<f64>::rem)
    }
}

impl Add for DynamicNumber {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Add::<i64>::add, Add::<f64>::add)
    }
}

impl AddAssign for DynamicNumber {
    fn add_assign(&mut self, rhs: Self) {
        match self {
            DynamicNumber::Float(a) => match rhs {
                DynamicNumber::Float(b) => *a += b,
                DynamicNumber::Integer(b) => *a += b as f64,
            },
            DynamicNumber::Integer(a) => match rhs {
                DynamicNumber::Float(b) => *self = DynamicNumber::Float((*a as f64) + b),
                DynamicNumber::Integer(b) => *a += b,
            },
        };
    }
}

impl Sub for DynamicNumber {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Sub::<i64>::sub, Sub::<f64>::sub)
    }
}

impl Mul for DynamicNumber {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        apply_op(self, rhs, Mul::<i64>::mul, Mul::<f64>::mul)
    }
}

impl Div for DynamicNumber {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        DynamicNumber::Float(match self {
            DynamicNumber::Integer(a) => match rhs {
                DynamicNumber::Integer(b) => a as f64 / b as f64,
                DynamicNumber::Float(b) => a as f64 / b,
            },
            DynamicNumber::Float(a) => match rhs {
                DynamicNumber::Integer(b) => a / b as f64,
                DynamicNumber::Float(b) => a / b,
            },
        })
    }
}

impl FromStr for DynamicNumber {
    type Err = ();

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<i64>() {
            Err(_) => match s.parse::<f64>() {
                Err(_) => Err(()),
                Ok(n) => Ok(DynamicNumber::Float(n)),
            },
            Ok(n) => Ok(DynamicNumber::Integer(n)),
        }
    }
}

impl TryFrom<&[u8]> for DynamicNumber {
    type Error = ();

    #[inline]
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match btoi::<i64>(value) {
            Ok(i) => Ok(DynamicNumber::Integer(i)),
            Err(_) => match fast_float::parse(value) {
                Ok(f) => Ok(DynamicNumber::Float(f)),
                Err(_) => Err(()),
            },
        }
    }
}

impl numfmt::Numeric for DynamicNumber {
    fn to_f64(&self) -> f64 {
        match self {
            Self::Float(n) => *n,
            Self::Integer(n) => *n as f64,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_number_ceil_floor_round() {
        assert_eq!(DynamicNumber::Float(2.3).ceil(), DynamicNumber::Integer(3));
        assert_eq!(DynamicNumber::Float(4.8).ceil(), DynamicNumber::Integer(5));
        assert_eq!(DynamicNumber::Integer(3).floor(), DynamicNumber::Integer(3));
        assert_eq!(DynamicNumber::Float(3.6).floor(), DynamicNumber::Integer(3));
        assert_eq!(
            DynamicNumber::Float(-3.6).floor(),
            DynamicNumber::Integer(-4)
        );
        assert_eq!(DynamicNumber::Integer(3).round(), DynamicNumber::Integer(3));
        assert_eq!(DynamicNumber::Float(3.6).round(), DynamicNumber::Integer(4));
        assert_eq!(DynamicNumber::Float(3.1).round(), DynamicNumber::Integer(3));
    }

    #[test]
    fn test_dynamic_number_ln_sqrt() {
        assert_eq!(DynamicNumber::Integer(1).ln(), DynamicNumber::Integer(0));
        assert_eq!(
            DynamicNumber::Float(3.5).ln(),
            DynamicNumber::Float(1.252762968495368)
        );
        assert_eq!(DynamicNumber::Integer(4).sqrt(), DynamicNumber::Integer(2));
        assert_eq!(
            DynamicNumber::Integer(100).sqrt(),
            DynamicNumber::Integer(10)
        );
    }

    #[test]
    fn test_dynamic_number_pow() {
        assert_eq!(
            DynamicNumber::Integer(2).pow(DynamicNumber::Integer(2)),
            DynamicNumber::Integer(4)
        );
    }
}
