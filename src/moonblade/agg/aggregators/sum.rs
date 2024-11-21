use crate::moonblade::types::DynamicNumber;

// NOTE: None means the sum means integer overflow
// NOTE: this sum implementation is using the Kahan-Babuska routine for precision
// Ref: https://en.wikipedia.org/wiki/Kahan_summation_algorithm
// Ref: https://github.com/simple-statistics/simple-statistics/blob/main/src/sum.js
#[derive(Debug, Clone)]
pub struct Sum {
    current: Option<DynamicNumber>,
    correction: f64,
}

impl Sum {
    pub fn new() -> Self {
        Self {
            current: Some(DynamicNumber::Integer(0)),
            correction: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.current = Some(DynamicNumber::Integer(0));
        self.correction = 0.0;
    }

    pub fn add(&mut self, value: DynamicNumber) {
        if let Some(current_sum) = self.current.as_mut() {
            match current_sum {
                DynamicNumber::Float(a) => match value {
                    DynamicNumber::Float(b) => {
                        let transition = *a + b;

                        if a.abs() > b.abs() {
                            self.correction += *a - transition + b;
                        } else {
                            self.correction += b - transition + *a;
                        }

                        *a = transition;
                    }
                    DynamicNumber::Integer(b) => {
                        let b = b as f64;

                        let transition = *a + b;

                        if a.abs() > b.abs() {
                            self.correction += *a - transition + b;
                        } else {
                            self.correction += b - transition + *a;
                        }

                        *a = transition;
                    }
                },
                DynamicNumber::Integer(a) => match value {
                    DynamicNumber::Float(b) => {
                        let a = *a as f64;

                        let transition = a + b;

                        if a.abs() > b.abs() {
                            self.correction += a - transition + b;
                        } else {
                            self.correction += b - transition + a;
                        }

                        self.current = Some(DynamicNumber::Float(transition));
                    }
                    DynamicNumber::Integer(b) => {
                        self.current = a.checked_add(b).map(DynamicNumber::Integer)
                    }
                },
            };
        }
    }

    pub fn get(&self) -> Option<DynamicNumber> {
        // NOTE: f64 overflow is a little bit more subtle
        match self.current {
            None => None,
            Some(sum) => match sum {
                DynamicNumber::Float(mut f) => {
                    f += self.correction;

                    if f == f64::MAX || f == f64::MIN || f.is_infinite() {
                        None
                    } else {
                        Some(DynamicNumber::Float(f))
                    }
                }
                _ => self.current,
            },
        }
    }

    pub fn merge(&mut self, other: Self) {
        if let Some(other_sum) = other.get() {
            self.add(other_sum);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kahan_babuska_summation() {
        let a = 10000.0;
        let b = 3.14159;
        let c = 2.71828;

        assert_eq!(a + b + c, 10005.859869999998);

        let mut sum = Sum::new();
        sum.add(DynamicNumber::Float(a));
        sum.add(DynamicNumber::Float(b));
        sum.add(DynamicNumber::Float(c));

        assert_eq!(sum.get(), Some(DynamicNumber::Float(10005.85987)));
    }
}
