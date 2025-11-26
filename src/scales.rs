use std::convert::TryFrom;
use std::fmt;
use std::mem;
use std::ops::Sub;

use colored::Colorize;
use colorgrad::{BasisGradient, Color, Gradient};
use jiff::{tz::TimeZone, Timestamp, Unit};

use crate::util;

// Taken straight from d3: https://github.com/d3/d3-array/blob/main/src/ticks.js
const E10: f64 = 7.0710678118654755; // sqrt(50)
const E5: f64 = 3.1622776601683795; // sqrt(10)
const E2: f64 = std::f64::consts::SQRT_2; // sqrt(2)

fn tick_spec(start: f64, stop: f64, count: f64) -> (f64, f64, f64) {
    let step = (stop - start) / count.max(0.0);
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

    if i2 < i1 && (0.5..2.0).contains(&count) {
        return tick_spec(start, stop, count * 2.0);
    }

    (i1, i2, inc)
}

fn tick_increment(start: f64, stop: f64, count: f64) -> f64 {
    tick_spec(start, stop, count).2
}

fn ticks(start: f64, stop: f64, count: f64) -> Vec<f64> {
    if count == 0.0 {
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
        step = tick_increment(start, stop, count as f64);

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

#[derive(Debug, Clone, Copy)]
pub struct Extent<T>((T, T));

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
    pub fn min(&self) -> T {
        self.0 .0
    }

    #[inline]
    pub fn max(&self) -> T {
        self.0 .1
    }

    pub fn into_inner(self) -> (T, T) {
        self.0
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

impl Extent<f64> {
    #[inline]
    fn lerp(&self, t: f64) -> f64 {
        lerp(self.min(), self.max(), t)
    }
}

impl<T: Copy + PartialOrd> From<(T, T)> for Extent<T> {
    fn from(value: (T, T)) -> Self {
        Self(value)
    }
}

// NOTE: this builder is also able to clamp values when processing them.
#[derive(Debug, Clone)]
pub struct ExtentBuilder<T> {
    extent: Option<Extent<T>>,
    min_clamp: Option<T>,
    max_clamp: Option<T>,
}

impl<T: Copy + PartialOrd> ExtentBuilder<T> {
    pub fn new() -> Self {
        Self {
            extent: None,
            max_clamp: None,
            min_clamp: None,
        }
    }

    pub fn clamp_min(&mut self, min: T) {
        self.min_clamp = Some(min);
    }

    pub fn clamp_max(&mut self, max: T) {
        self.max_clamp = Some(max);
    }

    pub fn process(&mut self, value: T) -> bool {
        if let Some(min) = self.min_clamp.as_ref() {
            if value < *min {
                return false;
            }
        }

        if let Some(max) = self.max_clamp.as_ref() {
            if value > *max {
                return false;
            }
        }

        match self.extent.as_mut() {
            None => {
                self.extent = Some(Extent::constant(value));
            }
            Some(extent) => extent.process(value),
        };

        true
    }

    pub fn build(self) -> Option<Extent<T>> {
        if let (Some(min), Some(max)) = (self.min_clamp, self.max_clamp) {
            return Some(Extent::from((min, max)));
        }

        self.extent.map(|mut extent| {
            if let Some(min_clamp) = self.min_clamp {
                extent.set_min(min_clamp);
            }
            if let Some(max_clamp) = self.max_clamp {
                extent.set_max(max_clamp);
            }

            extent
        })
    }
}

impl<T: Copy + PartialOrd> From<(Option<T>, Option<T>)> for ExtentBuilder<T> {
    fn from(value: (Option<T>, Option<T>)) -> Self {
        let mut extent_builder = Self::new();

        if let Some(min) = value.0 {
            extent_builder.clamp_min(min);
        }

        if let Some(max) = value.1 {
            extent_builder.clamp_max(max);
        }

        extent_builder
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
pub enum GradientName {
    // Sequential
    OrRd,
    Viridis,
    Inferno,
    Magma,
    Plasma,

    // Diverging
    BrBg,
    PiYg,
    PuOr,
    RdBu,
    RdGy,
    RdYlBu,
    RdYlGn,
    Spectral,
}

impl GradientName {
    pub fn as_str(&self) -> &str {
        use GradientName::*;

        match self {
            OrRd => "or_rd",
            Viridis => "viridis",
            Inferno => "inferno",
            Magma => "magma",
            Plasma => "plasma",
            BrBg => "br_bg",
            PiYg => "pi_yg",
            PuOr => "pu_or",
            RdBu => "rd_bu",
            RdGy => "rd_gy",
            RdYlBu => "rd_yl_bu",
            RdYlGn => "rd_yl_gn",
            Spectral => "spectral",
        }
    }

    pub fn build(&self) -> BasisGradient {
        use colorgrad::preset::*;
        use GradientName::*;

        match self {
            OrRd => or_rd(),
            Viridis => viridis(),
            Inferno => inferno(),
            Magma => magma(),
            Plasma => plasma(),
            BrBg => br_bg(),
            PiYg => pi_yg(),
            PuOr => pu_or(),
            RdBu => rd_bu(),
            RdGy => rd_gy(),
            RdYlBu => rd_yl_bu(),
            RdYlGn => rd_yl_gn(),
            Spectral => spectral(),
        }
    }

    pub fn sample(&self) -> String {
        self.build()
            .colors(30)
            .into_iter()
            .map(|c| {
                let rgb = c.to_rgba8();
                "  ".on_truecolor(rgb[0], rgb[1], rgb[2]).to_string()
            })
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn sequential_iter() -> impl Iterator<Item = Self> {
        use GradientName::*;
        [OrRd, Viridis, Inferno, Magma, Plasma].iter().copied()
    }

    pub fn diverging_iter() -> impl Iterator<Item = Self> {
        use GradientName::*;
        [BrBg, PiYg, PuOr, RdBu, RdGy, RdYlBu, RdYlGn, Spectral]
            .iter()
            .copied()
    }
}

impl TryFrom<String> for GradientName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use GradientName::*;

        Ok(match value.as_str() {
            "or_rd" => OrRd,
            "viridis" => Viridis,
            "inferno" => Inferno,
            "magma" => Magma,
            "plasma" => Plasma,
            "pi_yg" => PiYg,
            "pu_or" => PuOr,
            "rd_bu" => RdBu,
            "rd_gy" => RdGy,
            "rd_yl_bu" => RdYlBu,
            "rd_yl_gn" => RdYlGn,
            "spectral" => Spectral,
            _ => return Err(format!("unknown gradient \"{}\"", &value)),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogBase {
    Natural,
    Base2,
    Base10,
    Custom(f64),
}

impl LogBase {
    #[inline(always)]
    fn is_integer(&self) -> bool {
        match self {
            Self::Natural => false,
            Self::Base2 => true,
            Self::Base10 => true,
            Self::Custom(base) => base.fract() <= f64::EPSILON,
        }
    }

    #[inline(always)]
    fn as_float(&self) -> f64 {
        match self {
            Self::Natural => std::f64::consts::E,
            Self::Base2 => 2.0,
            Self::Base10 => 10.0,
            Self::Custom(base) => *base,
        }
    }

    #[inline(always)]
    fn apply(&self, x: f64) -> f64 {
        match self {
            Self::Natural => x.ln(),
            Self::Base2 => x.log2(),
            Self::Base10 => x.log10(),
            Self::Custom(base) => x.log(*base),
        }
    }

    #[inline(always)]
    fn invert(&self, x: f64) -> f64 {
        match self {
            Self::Natural => x.exp(),
            Self::Base2 => x.exp2(),
            Self::Base10 => 10.0_f64.powf(x),
            Self::Custom(base) => base.powf(x),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(try_from = "String")]
pub enum ScaleType {
    #[default]
    Linear,
    Logarithmic(LogBase),
}

impl ScaleType {
    pub fn is_linear(&self) -> bool {
        matches!(self, Self::Linear)
    }

    pub fn is_logarithmic(&self) -> bool {
        matches!(self, Self::Logarithmic(_))
    }
}

impl TryFrom<String> for ScaleType {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_str() {
            "lin" => Self::Linear,
            "log" | "ln" => Self::Logarithmic(LogBase::Natural),
            "log2" => Self::Logarithmic(LogBase::Base2),
            "log10" => Self::Logarithmic(LogBase::Base10),
            v if v.starts_with("log(") && v.ends_with(")") => {
                let base_str = v.split("log(").nth(1).unwrap().trim_end_matches(')');

                if let Ok(base) = base_str.parse::<f64>() {
                    Self::Logarithmic(LogBase::Custom(base))
                } else {
                    return Err(format!("could not parse log base \"{}\"", base_str));
                }
            }
            _ => return Err(format!("unknown scale type \"{}\"", &value)),
        })
    }
}

impl fmt::Display for ScaleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Linear => write!(f, "lin"),
            Self::Logarithmic(base) => match base {
                LogBase::Base2 => write!(f, "log2"),
                LogBase::Base10 => write!(f, "log10"),
                LogBase::Natural => write!(f, "log"),
                LogBase::Custom(base) => write!(f, "log({})", base),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct LinearScale {
    input_domain: Extent<f64>,
    output_range: Extent<f64>,
}

impl LinearScale {
    pub fn new(input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        assert!(input_domain.0 <= input_domain.1, "input_domain min > max");
        assert!(output_range.0 <= output_range.1, "output_range min > max");

        Self {
            input_domain: Extent::from(input_domain),
            output_range: Extent::from(output_range),
        }
    }

    pub fn from_extent(input_domain: Extent<f64>) -> Self {
        Self::new(input_domain.into_inner(), (0.0, 1.0))
    }

    pub fn nice(input_domain: (f64, f64), output_range: (f64, f64), ticks: usize) -> Self {
        Self::new(
            linear_nice(input_domain, ticks).unwrap_or(input_domain),
            output_range,
        )
    }

    #[inline]
    fn percent(&self, value: f64) -> f64 {
        (value - self.input_domain.min()) / self.input_domain.width()
    }

    pub fn map(&self, value: f64) -> f64 {
        let percent = self.percent(value);

        percent * self.output_range.width() + self.output_range.min()
    }

    pub fn map_color(&self, gradient: &BasisGradient, value: f64) -> Color {
        gradient.at(self.percent(value) as f32)
    }

    pub fn ticks(&self, count: usize) -> Vec<f64> {
        ticks(
            self.input_domain.min(),
            self.input_domain.max(),
            count as f64,
        )
    }

    fn formatted_ticks(&self, count: usize) -> Vec<String> {
        self.ticks(count)
            .into_iter()
            .map(util::format_number)
            .collect()
    }
}

fn format_timestamp(milliseconds: i64, unit: Unit) -> String {
    let timestamp = Timestamp::from_millisecond(milliseconds)
        .unwrap()
        .to_zoned(TimeZone::system());

    timestamp
        .strftime(match unit {
            Unit::Year => "%Y",
            Unit::Month => "%Y-%m",
            Unit::Day => "%F",
            _ => "%F %T",
        })
        .to_string()
}

#[derive(Debug, Clone)]
pub struct TimeScale {
    input_domain: Extent<f64>,
    output_range: Extent<f64>,
    unit: Unit,
}

impl TimeScale {
    fn new(input_domain: (f64, f64), output_range: (f64, f64), unit: Unit) -> Self {
        assert!(input_domain.0 <= input_domain.1, "input_domain min > max");
        assert!(output_range.0 <= output_range.1, "output_range min > max");

        Self {
            input_domain: Extent::from(input_domain),
            output_range: Extent::from(output_range),
            unit,
        }
    }

    fn nice(input_domain: (f64, f64), output_range: (f64, f64), unit: Unit) -> Self {
        Self::new(input_domain, output_range, unit)
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
        if count < 1 {
            return vec![];
        }

        if count < 3 {
            return vec![self.input_domain.min(), self.input_domain.max()];
        }

        let mut ticks = Vec::with_capacity(count);
        let mut t = 0.0;
        let fract = 1.0 / (count - 1) as f64;

        ticks.push(self.input_domain.min());

        for _ in 1..(count - 1) {
            t += fract;
            ticks.push(self.input_domain.lerp(t));
        }

        ticks.push(self.input_domain.max());

        ticks
    }

    fn formatted_ticks(&self, count: usize) -> Vec<String> {
        self.ticks(count)
            .into_iter()
            .map(|tick| format_timestamp(tick as i64, self.unit))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct LogScale {
    base: LogBase,
    input_domain: Extent<f64>,
    converted_input_domain: Extent<f64>,
    output_range: Extent<f64>,
}

impl LogScale {
    fn new(base: LogBase, input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        assert!(input_domain.0 <= input_domain.1, "input_domain min > max");
        assert!(output_range.0 <= output_range.1, "output_range min > max");

        Self {
            base,
            input_domain: Extent::from(input_domain),
            converted_input_domain: Extent::from((
                base.apply(input_domain.0),
                base.apply(input_domain.1),
            )),
            output_range: Extent::from(output_range),
        }
    }

    fn nice(base: LogBase, input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        let input_domain = (
            base.invert(base.apply(input_domain.0).floor()),
            base.invert(base.apply(input_domain.1).ceil()),
        );

        Self::new(base, input_domain, output_range)
    }

    #[inline]
    fn percent(&self, value: f64) -> f64 {
        (self.base.apply(value) - self.converted_input_domain.min())
            / self.converted_input_domain.width()
    }

    #[inline]
    fn map(&self, value: f64) -> f64 {
        let percent = self.percent(value);

        percent * self.output_range.width() + self.output_range.min()
    }

    // NOTE: I do not support reverse scales (i.e. pow scales?)
    fn ticks(&self, count: usize) -> Vec<f64> {
        let u = self.input_domain.min();
        let v = self.input_domain.max();

        let mut i = self.converted_input_domain.min();
        let mut j = self.converted_input_domain.max();

        if !self.base.is_integer() && j - i < count as f64 {
            i = i.floor();
            j = j.ceil();

            let mut alt_ticks = Vec::new();

            if u > 0.0 {
                while i <= j {
                    let mut k = 1.0;

                    while k < self.base.as_float() {
                        let t = if i < 0.0 {
                            k / self.base.invert(-i)
                        } else {
                            k * self.base.invert(i)
                        };

                        if t < u {
                            continue;
                        }

                        if t > v {
                            break;
                        }

                        alt_ticks.push(t);

                        k += 1.0;
                    }

                    i += 1.0;
                }
            } else {
                while i <= j {
                    let mut k = self.base.as_float() - 1.0;

                    while k >= 1.0 {
                        let t = if i > 0.0 {
                            k / self.base.invert(-i)
                        } else {
                            k * self.base.invert(i)
                        };

                        if t < u {
                            continue;
                        }

                        if t > v {
                            break;
                        }

                        alt_ticks.push(t);

                        k -= 1.0;
                    }

                    i += 1.0;
                }
            }

            if alt_ticks.len() * 2 < count {
                ticks(u, v, count as f64)
            } else {
                alt_ticks
            }
        } else {
            ticks(i, j, (j - i).min(count as f64))
                .into_iter()
                .map(|tick| self.base.invert(tick))
                .collect()
        }
    }

    fn formatted_ticks(&self, count: usize) -> Vec<String> {
        self.ticks(count)
            .into_iter()
            .map(util::format_number)
            .collect()
    }
}

#[derive(Debug, Clone)]
pub enum Scale {
    Linear(LinearScale),
    Log(LogScale),
    Time(TimeScale),
}

impl Scale {
    pub fn new(scale_type: ScaleType, input_domain: (f64, f64), output_range: (f64, f64)) -> Self {
        match scale_type {
            ScaleType::Linear => Self::Linear(LinearScale::new(input_domain, output_range)),
            ScaleType::Logarithmic(base) => {
                Self::Log(LogScale::new(base, input_domain, output_range))
            }
        }
    }

    pub fn nice(
        scale_type: ScaleType,
        input_domain: (f64, f64),
        output_range: (f64, f64),
        ticks: usize,
    ) -> Self {
        match scale_type {
            ScaleType::Linear => Self::Linear(LinearScale::nice(input_domain, output_range, ticks)),
            ScaleType::Logarithmic(base) => {
                Self::Log(LogScale::nice(base, input_domain, output_range))
            }
        }
    }

    pub fn time(input_domain: (f64, f64), output_range: (f64, f64), unit: Unit) -> Self {
        Self::Time(TimeScale::nice(input_domain, output_range, unit))
    }

    pub fn is_logarithmic(&self) -> bool {
        matches!(self, Self::Log(_))
    }

    pub fn formatted_ticks(&self, count: usize) -> Vec<String> {
        match self {
            Self::Linear(inner) => inner.formatted_ticks(count),
            Self::Log(inner) => inner.formatted_ticks(count),
            Self::Time(inner) => inner.formatted_ticks(count),
        }
    }

    pub fn percent(&self, value: f64) -> f64 {
        match self {
            Self::Linear(inner) => inner.percent(value),
            Self::Log(inner) => inner.percent(value),
            Self::Time(inner) => inner.percent(value),
        }
    }

    pub fn map(&self, value: f64) -> f64 {
        match self {
            Self::Linear(inner) => inner.map(value),
            Self::Log(inner) => inner.map(value),
            Self::Time(inner) => inner.map(value),
        }
    }
}

// pub struct HorizontalAxisOptions {
//     columns: usize,
//     left_offset: usize,
//     over: bool,
// }

// impl HorizontalAxisOptions {
//     fn available_width(&self) -> usize {
//         self.left_offset + self.columns
//     }

//     fn capacity(&self) -> usize {
//         self.available_width() * 2 + 1
//     }
// }

// impl Scale {
//     pub fn draw_horizontal_axis(&self, options: HorizontalAxisOptions) -> String {
//         let mut output = String::with_capacity(options.capacity());

//         output
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticks() {
        assert_eq!(ticks(0.0, 10.0, 0.0), Vec::<f64>::new());
        assert_eq!(ticks(1.0, 1.0, 10.0), vec![1.0]);
        assert_eq!(ticks(0.0, 10.0, 1.0), vec![0.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 2.0), vec![0.0, 5.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 3.0), vec![0.0, 5.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 4.0), vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(ticks(0.0, 10.0, 5.0), [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(
            ticks(0.0, 10.0, 10.0),
            [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        );
        assert_eq!(ticks(-5.0, 5.0, 3.0), vec![-5.0, 0.0, 5.0]);
    }

    #[test]
    fn test_linear_nice() {
        assert_eq!(linear_nice((0.0, 10.0), 10), Some((0.0, 10.0)));
        assert_eq!(linear_nice((0.0, 10.0), 17), Some((0.0, 10.0)));
        assert_eq!(linear_nice((0.4868, 10.85), 10), Some((0.0, 11.0)));
    }
}
