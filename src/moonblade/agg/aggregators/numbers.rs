use rayon::prelude::*;

use crate::moonblade::types::DynamicNumber;

static SPARKLINE_BLOCKS: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇']; // '█'

#[derive(Debug, Clone)]
pub enum MedianType {
    Interpolation,
    Low,
    High,
}

#[derive(Debug, Clone)]
pub struct Numbers {
    numbers: Vec<DynamicNumber>,
}

impl Numbers {
    pub fn new() -> Self {
        Self {
            numbers: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.numbers.clear();
    }

    pub fn add(&mut self, number: DynamicNumber) {
        self.numbers.push(number);
    }

    pub fn finalize(&mut self, parallel: bool) {
        let cmp = |a: &DynamicNumber, b: &DynamicNumber| a.partial_cmp(b).unwrap();

        if parallel {
            self.numbers.par_sort_unstable_by(cmp);
        } else {
            self.numbers.sort_unstable_by(cmp);
        }
    }

    pub fn median(&self, median_type: &MedianType) -> Option<DynamicNumber> {
        let count = self.numbers.len();

        if count == 0 {
            return None;
        }

        let median = match median_type {
            MedianType::Low => {
                let mut midpoint = count / 2;

                if count % 2 == 0 {
                    midpoint -= 1;
                }

                self.numbers[midpoint]
            }
            MedianType::High => {
                let midpoint = count / 2;

                self.numbers[midpoint]
            }
            MedianType::Interpolation => {
                let midpoint = count / 2;

                if count % 2 == 1 {
                    self.numbers[midpoint]
                } else {
                    let down = &self.numbers[midpoint - 1];
                    let up = &self.numbers[midpoint];

                    (*down + *up) / DynamicNumber::Float(2.0)
                }
            }
        };

        Some(median)
    }

    // NOTE: using the inclusive method from https://github.com/python/cpython/blob/3.12/Lib/statistics.py
    pub fn quantiles(&self, n: usize) -> Option<Vec<DynamicNumber>> {
        let l = self.numbers.len();

        if l < 2 {
            return None;
        }

        let mut result: Vec<DynamicNumber> = Vec::new();

        let m = l - 1;

        for i in 1..n {
            let c = i * m;
            let j = c.div_euclid(n);
            let delta = c.rem_euclid(n);

            let interpolated = (self.numbers[j] * DynamicNumber::Integer((n - delta) as i64)
                + self.numbers[j + 1] * DynamicNumber::Integer(delta as i64))
                / DynamicNumber::Integer(n as i64);

            result.push(interpolated);
        }

        Some(result)
    }

    pub fn quartiles(&self) -> Option<Vec<DynamicNumber>> {
        self.quantiles(4)
    }

    // NOTE: from https://github.com/simple-statistics/simple-statistics/blob/main/src/quantile_sorted.js
    pub fn quantile(&self, p: f64) -> Option<DynamicNumber> {
        let n = &self.numbers;
        let l = n.len();

        if !(0.0..=1.0).contains(&p) {
            None
        } else if p == 1.0 {
            Some(n[l - 1])
        } else if p == 0.0 {
            Some(n[0])
        } else {
            let idx = (l as f64) * p;

            if idx.fract() != 0.0 {
                Some(n[idx.ceil() as usize - 1])
            } else {
                let idx = idx.floor() as usize;

                if l % 2 == 0 {
                    Some((n[idx - 1] + n[idx]) / DynamicNumber::Integer(2))
                } else {
                    Some(n[idx])
                }
            }
        }
    }

    pub fn sparkline(&self, bins: usize) -> String {
        if self.numbers.is_empty() {
            return " ".repeat(bins);
        }

        if self.numbers.len() == 1 {
            return "▇".to_string() + &" ".repeat(bins.saturating_sub(1));
        }

        let min = *self.numbers.first().unwrap();
        let max = *self.numbers.last().unwrap();
        let width = max - min;
        let cell_width = width.as_float() / bins as f64;

        let mut cells = vec![0usize; bins];

        for number in self.numbers.iter().copied() {
            let mut cell_index = ((number - min).as_float() / cell_width).floor() as usize;

            if cell_index == bins {
                cell_index -= 1;
            }

            debug_assert!(cell_index < bins);

            cells[cell_index] += 1;
        }

        let mut line = String::with_capacity(bins);
        let n = self.numbers.len() as f64;

        for cell in cells {
            let sparkline_block_index = (cell as f64 / n * 7.0).ceil() as usize;
            line.push(SPARKLINE_BLOCKS[sparkline_block_index]);
        }

        line
    }

    pub fn merge(&mut self, other: Self) {
        self.numbers.extend(other.numbers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl From<Vec<usize>> for Numbers {
        fn from(values: Vec<usize>) -> Self {
            let mut numbers = Self::new();

            for n in values {
                numbers.add(DynamicNumber::Integer(n as i64));
            }

            numbers
        }
    }

    #[test]
    fn test_median_aggregator() {
        let odd = vec![1, 3, 5];
        let even = vec![1, 2, 6, 7];

        let mut no_numbers = Numbers::new();
        let mut lone_numbers = Numbers::from(vec![8]);
        let mut odd_numbers = Numbers::from(odd);
        let mut even_numbers = Numbers::from(even);

        no_numbers.finalize(false);
        lone_numbers.finalize(false);
        odd_numbers.finalize(false);
        even_numbers.finalize(false);

        // Low
        assert_eq!(no_numbers.median(&MedianType::Low), None);

        assert_eq!(
            lone_numbers.median(&MedianType::Low),
            Some(DynamicNumber::Integer(8))
        );

        assert_eq!(
            odd_numbers.median(&MedianType::Low),
            Some(DynamicNumber::Integer(3))
        );

        assert_eq!(
            even_numbers.median(&MedianType::Low),
            Some(DynamicNumber::Integer(2))
        );

        // High
        assert_eq!(no_numbers.median(&MedianType::High), None);

        assert_eq!(
            lone_numbers.median(&MedianType::High),
            Some(DynamicNumber::Integer(8))
        );

        assert_eq!(
            odd_numbers.median(&MedianType::High),
            Some(DynamicNumber::Integer(3))
        );

        assert_eq!(
            even_numbers.median(&MedianType::High),
            Some(DynamicNumber::Integer(6))
        );

        // High
        assert_eq!(no_numbers.median(&MedianType::Interpolation), None);

        assert_eq!(
            lone_numbers.median(&MedianType::Interpolation),
            Some(DynamicNumber::Integer(8))
        );

        assert_eq!(
            odd_numbers.median(&MedianType::Interpolation),
            Some(DynamicNumber::Integer(3))
        );

        assert_eq!(
            even_numbers.median(&MedianType::Interpolation),
            Some(DynamicNumber::Float(4.0))
        );

        // Quartiles
        fn manual_quartiles(n: &Numbers) -> Option<Vec<DynamicNumber>> {
            Some(vec![
                n.quantile(0.25).unwrap(),
                n.quantile(0.5).unwrap(),
                n.quantile(0.75).unwrap(),
            ])
        }

        assert_eq!(
            even_numbers.quartiles(),
            Some(vec![
                DynamicNumber::Float(1.75),
                DynamicNumber::Float(4.0),
                DynamicNumber::Float(6.25)
            ])
        );
        assert_eq!(
            manual_quartiles(&even_numbers),
            Some(vec![
                DynamicNumber::Float(1.5),
                DynamicNumber::Float(4.0),
                DynamicNumber::Float(6.5)
            ])
        );

        assert_eq!(
            odd_numbers.quartiles(),
            Some(vec![
                DynamicNumber::Float(2.0),
                DynamicNumber::Float(3.0),
                DynamicNumber::Float(4.0)
            ])
        );
        assert_eq!(
            manual_quartiles(&odd_numbers),
            Some(vec![
                DynamicNumber::Integer(1),
                DynamicNumber::Integer(3),
                DynamicNumber::Integer(5)
            ])
        );
    }
}
