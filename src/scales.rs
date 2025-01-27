use std::mem;

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

// NOTE: same as d3-scale's linear nice
fn improve_domain_for_ticks(domain: (f64, f64), count: usize) -> Option<(f64, f64)> {
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
enum Conversion {
    #[default]
    Linear,
    Ln,
}

impl Conversion {
    fn linear() -> Self {
        Self::Linear
    }

    fn ln() -> Self {
        Self::Ln
    }

    fn is_linear(&self) -> bool {
        matches!(self, Self::Linear)
    }

    #[inline]
    fn convert(&self, x: f64) -> f64 {
        match self {
            Self::Linear => x,
            Self::Ln => x.ln(),
        }
    }

    #[inline]
    fn invert(&self, x: f64) -> f64 {
        match self {
            Self::Linear => x,
            Self::Ln => x.exp(),
        }
    }
}

#[derive(Debug)]
pub struct Scale {
    input_domain: (f64, f64),
    output_range: (f64, f64),
    conversion: Conversion,
}

impl Scale {
    fn new(input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        assert!(input_domain.0 <= input_domain.1, "input_domain min > max");
        assert!(output_range.0 <= output_range.1, "output_range min > max");

        Self {
            input_domain,
            output_range,
            conversion: Default::default(),
        }
    }

    fn nice(input_domain: (f64, f64), output_range: (f64, f64), ticks: usize) -> Self {
        Self::new(
            improve_domain_for_ticks(input_domain, ticks).unwrap_or(input_domain),
            output_range,
        )
    }

    #[inline]
    fn lerp(&self, t: f64) -> f64 {
        (1.0 - t) * self.output_range.0 + t * self.output_range.1
    }

    #[inline]
    fn input_domain_width(&self) -> f64 {
        self.input_domain.1 - self.input_domain.0
    }

    #[inline]
    fn output_range_width(&self) -> f64 {
        self.output_range.1 - self.output_range.0
    }

    #[inline]
    fn percent(&self, value: f64) -> f64 {
        (value - self.input_domain.0) / self.input_domain_width()
    }

    fn map(&self, value: f64) -> f64 {
        let percent = self.percent(value);

        percent * self.output_range_width() + self.output_range.0
    }

    fn spread_input_domain(&mut self, offset: f64) {
        self.input_domain.0 -= offset;
        self.input_domain.1 += offset;
    }

    fn ticks(&self, count: usize) -> Vec<f64> {
        ticks(self.input_domain.0, self.input_domain.1, count)
            .into_iter()
            .map(|tick| self.conversion.convert(tick))
            .collect()
    }
}

// TODO: linear, log etc. with different struct to process and one belonging to the scale
// TODO: d3 style nice
// TODO: extent builder with optional custom bounds?
// TODO: convert, invert
// TODO: ticks, continuous or not

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
    fn test_improve_domain_for_ticks() {
        assert_eq!(improve_domain_for_ticks((0.0, 10.0), 10), Some((0.0, 10.0)));
        assert_eq!(improve_domain_for_ticks((0.0, 10.0), 17), Some((0.0, 10.0)));
        assert_eq!(
            improve_domain_for_ticks((0.4868, 10.85), 10),
            Some((0.0, 11.0))
        );
    }
}
