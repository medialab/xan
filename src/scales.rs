use std::mem;
use std::ops::Sub;

use serde::de::{Deserialize, Deserializer, Error};

// Taken straight from d3: https://github.com/d3/d3-array/blob/main/src/ticks.js
const E10: f64 = 7.0710678118654755; // sqrt(50)
const E5: f64 = 3.1622776601683795; // sqrt(10)
const E2: f64 = std::f64::consts::SQRT_2; // sqrt(2)

fn tick_spec(start: f64, stop: f64, count: usize) -> (f64, f64, f64) {
    let step = (stop - start) / count.max(0) as f64;
    let power = step.log10().floor();
    let error = step / 10.0_f64.powf(power);
    let factor = if error >= E10 {
        10.0
    } else if error >= E5 {
        5.0
    } else if error >= E2 {
        2.0
    } else {
        1.0
    };

    let (i1, i2, inc) = if power < 0.0 {
        let mut inc = 10.0_f64.powf(-power) / factor;
        let mut i1 = (start * inc).round();
        let mut i2 = (stop * inc).round();
        if i1 / inc < start {
            i1 += 1.0;
        }
        if i2 / inc > stop {
            i2 -= 1.0;
        }
        inc = -inc;

        (i1, i2, inc)
    } else {
        let inc = 10.0_f64.powf(power) * factor;
        let mut i1 = (start / inc).round();
        let mut i2 = (stop / inc).round();
        if i1 * inc < start {
            i1 += 1.0;
        }
        if i2 * inc > stop {
            i2 -= 1.0;
        }

        (i1, i2, inc)
    };

    if i2 < i1 && 0.5 <= count as f64 && count < 2 {
        return tick_spec(start, stop, count * 2);
    }

    (i1, i2, inc)
}

fn tick_increment(start: f64, stop: f64, count: usize) -> f64 {
    tick_spec(start, stop, count).2
}

fn ticks(start: f64, stop: f64, count: usize) -> Vec<f64> {
    if count == 0 {
        return vec![];
    }

    if start == stop {
        return vec![start];
    }

    let reverse = stop < start;

    let (i1, i2, inc) = if reverse {
        tick_spec(stop, start, count)
    } else {
        tick_spec(start, stop, count)
    };

    if i2 < i1 {
        return vec![];
    }

    let n = (i2 - i1 + 1.0) as usize;

    let mut ticks = Vec::with_capacity(n);

    if reverse {
        if inc < 0.0 {
            for i in 0..n {
                ticks.push((i2 - i as f64) / -inc);
            }
        } else {
            for i in 0..n {
                ticks.push((i2 - i as f64) * inc);
            }
        }
    } else if inc < 0.0 {
        for i in 0..n {
            ticks.push((i1 + i as f64) / -inc);
        }
    } else {
        for i in 0..n {
            ticks.push((i1 + i as f64) * inc);
        }
    }

    ticks
}

fn linear_nice(domain: (f64, f64), count: usize) -> Option<(f64, f64)> {
    let (mut start, mut stop) = domain;

    let mut step: f64;
    let mut previous_step: Option<f64> = None;

    if stop < start {
        mem::swap(&mut start, &mut stop);
    }

    for _ in 0..10 {
        step = tick_increment(start, stop, count);

        if matches!(previous_step, Some(s) if s == step) {
            return Some(if stop < start {
                (stop, start)
            } else {
                (start, stop)
            });
        }

        if step > 0.0 {
            start = (start / step).floor() * step;
            stop = (stop / step).ceil() * step;
        } else if step < 0.0 {
            start = (start * step).ceil() / step;
            stop = (stop * step).floor() / step;
        } else {
            break;
        }

        previous_step = Some(step);
    }

    None
}

#[inline]
fn lerp(min: f64, max: f64, t: f64) -> f64 {
    (1.0 - t) * min + t * max
}

#[derive(Debug)]
struct Extent<T>((T, T));

impl<T: Copy + PartialOrd> Extent<T> {
    fn constant(value: T) -> Self {
        Self((value, value))
    }

    #[inline]
    fn set_min(&mut self, value: T) {
        self.0 .0 = value;
    }

    #[inline]
    fn set_max(&mut self, value: T) {
        self.0 .1 = value;
    }

    #[inline]
    fn min(&self) -> T {
        self.0 .0
    }

    #[inline]
    fn max(&self) -> T {
        self.0 .1
    }

    fn process(&mut self, value: T) {
        if value < self.0 .0 {
            self.0 .0 = value;
        }

        if value > self.0 .1 {
            self.0 .1 = value;
        }
    }
}

impl<T: Copy + PartialOrd + Sub<Output = T>> Extent<T> {
    #[inline]
    fn width(&self) -> T {
        self.max() - self.min()
    }
}

impl<T: Copy + PartialOrd> From<(T, T)> for Extent<T> {
    fn from(value: (T, T)) -> Self {
        Self(value)
    }
}

// TODO: support known/unknown min/max beforehand and make process
// return bool so we can use it to clamp in heatmap, and bins
#[derive(Debug)]
struct ExtentBuilder<T>(Option<Extent<T>>);

impl<T: Copy + PartialOrd> ExtentBuilder<T> {
    fn new() -> Self {
        Self(None)
    }

    fn process(&mut self, value: T) {
        match self.0.as_mut() {
            None => {
                self.0 = Some(Extent::constant(value));
            }
            Some(extent) => extent.process(value),
        };
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ScaleType {
    #[default]
    Linear,
    Log10,
}

impl ScaleType {
    pub fn is_linear(&self) -> bool {
        matches!(self, Self::Linear)
    }

    #[inline]
    pub fn convert(&self, x: f64) -> f64 {
        match self {
            Self::Linear => x,
            Self::Log10 => x.log10(),
        }
    }

    #[inline]
    fn invert(&self, x: f64) -> f64 {
        match self {
            Self::Linear => x,
            Self::Log10 => 10.0_f64.powf(x),
        }
    }
}

impl<'de> Deserialize<'de> for ScaleType {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;

        Ok(match raw.as_str() {
            "lin" => Self::Linear,
            "log" => Self::Log10,
            _ => return Err(D::Error::custom(format!("unknown scale type \"{}\"", raw))),
        })
    }
}

#[derive(Debug)]
pub struct LinearScale {
    input_domain: Extent<f64>,
    output_range: Extent<f64>,
}

impl LinearScale {
    fn new(input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        assert!(input_domain.0 <= input_domain.1, "input_domain min > max");
        assert!(output_range.0 <= output_range.1, "output_range min > max");

        Self {
            input_domain: Extent::from(input_domain),
            output_range: Extent::from(output_range),
        }
    }

    fn nice(input_domain: (f64, f64), output_range: (f64, f64), ticks: usize) -> Self {
        Self::new(
            linear_nice(input_domain, ticks).unwrap_or(input_domain),
            output_range,
        )
    }

    #[inline]
    fn percent(&self, value: f64) -> f64 {
        (value - self.input_domain.min()) / self.input_domain.width()
    }

    fn map(&self, value: f64) -> f64 {
        let percent = self.percent(value);

        percent * self.output_range.width() + self.output_range.min()
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        ticks(self.input_domain.min(), self.input_domain.max(), count)
    }
}

// TODO: support custom base, currently only base10
struct LogScale {
    input_domain: Extent<f64>,
    output_range: Extent<f64>,
}

impl LogScale {
    fn new(input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        assert!(input_domain.0 <= input_domain.1, "input_domain min > max");
        assert!(output_range.0 <= output_range.1, "output_range min > max");

        Self {
            input_domain: Extent::from(input_domain),
            output_range: Extent::from(output_range),
        }
    }

    fn nice(input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        let input_domain = (
            10.0_f64.powf(input_domain.0.log10().floor()),
            10.0_f64.powf(input_domain.1.log10().ceil()),
        );

        Self::new(input_domain, output_range)
    }

    // NOTE: I do not support reverse scales (i.e. pow scales?)
    fn ticks(&self, count: usize) -> Vec<f64> {
        let u = self.input_domain.min();
        let v = self.input_domain.max();

        let i = u.log10();
        let j = v.log10();

        // NOTE: I do not support non int bases either like d3
        ticks(i, j, ((j - i).floor() as usize).min(count))
            .into_iter()
            .map(|tick| 10.0_f64.powf(tick))
            .collect()
    }
}

// TODO: log scale, enum Scale buildable from ScaleType
// TODO: extent builder

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticks() {
        assert_eq!(ticks(0.0, 10.0, 0), Vec::<f64>::new());
        assert_eq!(ticks(1.0, 1.0, 10), vec![1.0]);
        assert_eq!(ticks(0.0, 10.0, 1), vec![0.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 2), vec![0.0, 5.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 3), vec![0.0, 5.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 4), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 5), [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(
            ticks(0.0, 10.0, 10),
            [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        );
        assert_eq!(ticks(-5.0, 5.0, 3), vec![-5.0, 0.0, 5.0]);
    }

    #[test]
    fn test_linear_nice() {
        assert_eq!(linear_nice((0.0, 10.0), 10), Some((0.0, 10.0)));
        assert_eq!(linear_nice((0.0, 10.0), 17), Some((0.0, 10.0)));
        assert_eq!(linear_nice((0.4868, 10.85), 10), Some((0.0, 11.0)));
    }
}
