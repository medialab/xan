use std::fmt::{Display, Write as FmtWrite};
use std::io::{Write, stdout};
use std::num::NonZeroUsize;

use colored::Colorize;
use colorgrad::Gradient;
use jiff::{Timestamp, Unit, tz::TimeZone};
use ordered_float::NotNan;
use pad::{Alignment, PadStr};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use simd_csv::ByteRecord;
use unicode_width::UnicodeWidthStr;

use crate::CliResult;
use crate::cmd::heatmap::CramMode;
use crate::cmd::plot::{Aggregation, Granularity};
use crate::cmd::stats::linear_time_median;
use crate::collections::{ClusteredInsertHashmap, IndexMap, new_index_map};
use crate::config::{Config, Delimiter};
use crate::moonblade::{TemporalExtent, Welford};
use crate::scales::{Extent, ExtentBuilder, GradientName, HistogramBuilder, Scale, ScaleType};
use crate::select::{SelectedColumns, Selection};
use crate::temporal::{FuzzyTemporal, TimestampExt, ZonedExt, parse_fuzzy_temporal};
use crate::util::{self, ColorMode, ColorOrStyles, format_number};

fn compute_name_hash(name: &[u8]) -> usize {
    let mut sum: usize = 0;

    for byte in name {
        sum += *byte as usize;
    }

    sum
}

fn parse_temporal(cell: &[u8]) -> CliResult<(FuzzyTemporal, f64)> {
    let fuzzy_temporal = parse_fuzzy_temporal(cell, true)?;
    let timestamp = fuzzy_temporal.to_lower_bound_timestamp(TimeZone::system())?;
    Ok((fuzzy_temporal, timestamp.as_duration().as_secs_f64()))
}

fn build_seconds_extent(extent: &TemporalExtent, unit: Unit) -> CliResult<Extent<f64>> {
    let earliest = FuzzyTemporal::from(extent.earliest().unwrap());
    let latest = FuzzyTemporal::from(extent.latest().unwrap());

    let earliest_seconds = earliest
        .to_lower_bound_timestamp(TimeZone::system())?
        .to_zoned(TimeZone::system())
        .floor(unit)?
        .timestamp()
        .as_duration()
        .as_secs_f64();

    let latest_seconds = latest
        .to_lower_bound_timestamp(TimeZone::system())?
        .to_zoned(TimeZone::system())
        .floor(unit)?
        .timestamp()
        .as_duration()
        .as_secs_f64();

    Ok(Extent::from((earliest_seconds, latest_seconds)))
}

fn fill_discretization_gaps<T: Default + Copy, F>(
    bins: &mut [Option<T>],
    max_gap: usize,
    coalesce: F,
) where
    F: Fn(T, T, f64) -> T,
{
    let len = bins.len();

    let mut i = 0;

    while i < len {
        if bins[i].is_none() {
            let right = (i..len).find(|&j| bins[j].is_some());
            let gap_len = right.unwrap_or(len) - i;

            if gap_len <= max_gap {
                let left_value = (0..i).rev().find_map(|j| bins[j]).unwrap_or_default();
                let right_value = right.and_then(|j| bins[j]).unwrap_or_default();

                for (k, bin) in bins.iter_mut().enumerate().skip(i).take(gap_len) {
                    let t = (k - i + 1) as f64 / (gap_len + 1) as f64;
                    *bin = Some(coalesce(left_value, right_value, t));
                }
            }

            i += gap_len.max(1);
        } else {
            i += 1;
        }
    }
}

pub static SPARKLINE_CHARS: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];
pub const FULL_BAR: char = '█';

#[derive(Default, Clone)]
enum SparklineColorMode {
    #[default]
    None,
    Striped,
    Rainbow,
    StripedRainbow,
    Gradient(Box<dyn Gradient>, bool),
    BackgroundGradient(Box<dyn Gradient>),
}

impl SparklineColorMode {
    fn is_background_gradient(&self) -> bool {
        matches!(self, Self::BackgroundGradient(_))
    }
}

pub struct SparklineRendererOptions {
    pub height: usize,
    pub width: usize,
    color_mode: SparklineColorMode,
}

impl Default for SparklineRendererOptions {
    fn default() -> Self {
        Self {
            height: 1,
            width: 1,
            color_mode: SparklineColorMode::default(),
        }
    }
}

impl SparklineRendererOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_striped(&mut self) {
        self.color_mode = SparklineColorMode::Striped;
    }

    pub fn build(self) -> SparklineRenderer {
        SparklineRenderer {
            draw_buffer: String::new(),
            options: self,
        }
    }
}

pub struct SparklineRenderer {
    draw_buffer: String,
    options: SparklineRendererOptions,
}

impl SparklineRenderer {
    #[inline(always)]
    pub fn render(&mut self, scale: &Scale, bins: &[f64]) {
        self.render_impl(0, None, scale, bins, None);
    }

    #[inline(always)]
    pub fn render_with_color_overrides(
        &mut self,
        scale: &Scale,
        bins: &[f64],
        color_overrides_opt: Option<&[Option<ColorOrStyles>]>,
    ) {
        self.render_impl(0, None, scale, bins, color_overrides_opt);
    }

    pub fn render_impl(
        &mut self,
        sparkline_index: usize,
        name_opt: Option<&str>,
        scale: &Scale,
        bins: &[f64],
        color_overrides_opt: Option<&[Option<ColorOrStyles>]>,
    ) {
        let name_width = name_opt.map(|name| name.width()).unwrap_or(0);

        let height = self.options.height;
        let width = self.options.width;

        self.draw_buffer.clear();

        for h in (0..height).rev() {
            match (h, name_opt) {
                (0, Some(name)) => {
                    self.draw_buffer.push_str(name);
                }
                (_, Some(_)) => {
                    for _ in 0..name_width {
                        self.draw_buffer.push(' ');
                    }
                }
                _ => (),
            };

            let len = SPARKLINE_CHARS.len();

            for (i, y) in bins.iter().copied().enumerate() {
                let ratio = if y == 0.0 { 0.0 } else { scale.ratio(y) };

                let sparkline_char = if self.options.color_mode.is_background_gradient() || y == 0.0
                {
                    ' '
                } else {
                    let scaled = ratio * height as f64;

                    let full = scaled.floor() as usize;
                    let frac = scaled - full as f64;

                    if full > h {
                        if h == height - 1 {
                            SPARKLINE_CHARS[len - 1]
                        } else {
                            FULL_BAR
                        }
                    } else if full == h && frac > 1e-9 {
                        let mut bar_index = (frac * len as f64).ceil() as usize;
                        bar_index = bar_index.saturating_sub(1).min(len - 1);

                        SPARKLINE_CHARS[bar_index]
                    } else if h == 0 {
                        SPARKLINE_CHARS[0]
                    } else {
                        ' '
                    }
                };

                for _ in 0..width {
                    match color_overrides_opt {
                        Some(color_overrides)
                            if matches!(color_overrides.get(i), Some(Some(_))) =>
                        {
                            let color = color_overrides[i].unwrap();

                            write!(
                                &mut self.draw_buffer,
                                "{}",
                                color.colorize(&sparkline_char.to_string())
                            )
                            .unwrap();
                        }
                        _ => match &self.options.color_mode {
                            SparklineColorMode::None => {
                                self.draw_buffer.push(sparkline_char);
                            }
                            SparklineColorMode::Striped => {
                                if i % 2 == 0 {
                                    write!(
                                        &mut self.draw_buffer,
                                        "{}",
                                        sparkline_char.to_string().dimmed()
                                    )
                                    .unwrap();
                                } else {
                                    self.draw_buffer.push(sparkline_char);
                                }
                            }
                            SparklineColorMode::Rainbow => {
                                let color = util::colorizer_by_rainbow(sparkline_index, "spark");

                                write!(
                                    &mut self.draw_buffer,
                                    "{}",
                                    color.colorize(&sparkline_char.to_string())
                                )
                                .unwrap();
                            }
                            SparklineColorMode::StripedRainbow => {
                                let color = util::colorizer_by_rainbow(sparkline_index, "spark");

                                if i % 2 == 0 {
                                    write!(
                                        &mut self.draw_buffer,
                                        "{}",
                                        color.colorize(&sparkline_char.to_string()).dimmed()
                                    )
                                    .unwrap();
                                } else {
                                    write!(
                                        &mut self.draw_buffer,
                                        "{}",
                                        color.colorize(&sparkline_char.to_string())
                                    )
                                    .unwrap();
                                }
                            }
                            SparklineColorMode::Gradient(gradient, vertical) => {
                                let c = gradient
                                    .at(if *vertical {
                                        (h + 1) as f32 / height as f32
                                    } else {
                                        ratio as f32
                                    })
                                    .to_rgba8();

                                write!(
                                    &mut self.draw_buffer,
                                    "{}",
                                    sparkline_char.to_string().truecolor(c[0], c[1], c[2])
                                )
                                .unwrap();
                            }
                            SparklineColorMode::BackgroundGradient(gradient) => {
                                let c = gradient.at(ratio as f32).to_rgba8();

                                write!(
                                    &mut self.draw_buffer,
                                    "{}",
                                    sparkline_char.to_string().on_truecolor(c[0], c[1], c[2])
                                )
                                .unwrap();
                            }
                        },
                    };
                }
            }

            self.draw_buffer.push('\n');
        }

        // NOTE: removing last newline
        self.draw_buffer.pop();
    }

    fn pad_left(&mut self, padding: usize) {
        for _ in 0..padding {
            self.draw_buffer.push(' ');
        }
    }

    fn cram_names<'a>(&mut self, left_padding: usize, names: impl Iterator<Item = &'a [u8]>) {
        self.draw_buffer.push('\n');

        self.pad_left(left_padding);

        let width = self.options.width;

        for name in names {
            self.draw_buffer.push_str(
                &util::unicode_aware_ellipsis(
                    &util::sanitize_text_for_single_line_printing(&String::from_utf8_lossy(name)),
                    width,
                )
                .pad_to_width_with_alignment(width, Alignment::Middle)
                .dimmed()
                .to_string(),
            );
        }
    }

    fn show_numbers(
        &mut self,
        left_padding: usize,
        numbers: &[f64],
        percentage_denominator: Option<f64>,
    ) {
        self.draw_buffer.push('\n');

        self.pad_left(left_padding);

        let width = self.options.width;

        for x in numbers.iter().copied() {
            self.draw_buffer.push_str(
                &util::unicode_aware_ellipsis(
                    &(if let Some(denominator) = percentage_denominator {
                        let percentage = if denominator == 0.0 {
                            0.0
                        } else {
                            x / denominator * 100.0
                        };
                        format!("{:3.0}%", percentage)
                    } else {
                        util::format_number(x)
                    }),
                    width,
                )
                .pad_to_width_with_alignment(width, Alignment::Middle),
            );
        }
    }

    pub fn render_central_tendency(
        &mut self,
        left_padding: usize,
        bins: usize,
        mean_index: usize,
        median_index: usize,
        sigma_left_index: usize,
        sigma_right_index: usize,
    ) {
        self.draw_buffer.push('\n');

        self.pad_left(left_padding);

        for i in 0..bins {
            if i == mean_index {
                write!(&mut self.draw_buffer, "{}", "━".green()).unwrap();
            } else if i == median_index {
                write!(&mut self.draw_buffer, "{}", "━".yellow()).unwrap();
            } else if i == sigma_left_index {
                write!(&mut self.draw_buffer, "{}", "<".magenta()).unwrap();
            } else if i == sigma_right_index {
                write!(&mut self.draw_buffer, "{}", ">".magenta()).unwrap();
            } else {
                self.draw_buffer.push(' ');
            }
        }
    }

    fn render_distribution_legend(&mut self, left_padding: usize, info: &DistributionInfo) {
        let (mean_index, median_index, sigma_left_index, sigma_right_index) =
            info.central_tendency_indices;

        self.render_central_tendency(
            left_padding,
            info.bins,
            mean_index,
            median_index,
            sigma_left_index,
            sigma_right_index,
        );

        self.draw_buffer.push('\n');
        self.pad_left(left_padding);

        writeln!(
            &mut self.draw_buffer,
            "highest: {}/{} ({:.2}%), min: {}, max: {}",
            format_number(info.max_bin_count).red(),
            format_number(info.count),
            (info.max_bin_count as f64 / info.count as f64) * 100.0,
            format_number(info.min).blue(),
            format_number(info.max).red(),
        )
        .unwrap();

        self.pad_left(left_padding);

        writeln!(
            &mut self.draw_buffer,
            "mean: {}, median: {}, stddev: {}",
            format_number(info.mean).green(),
            format_number(info.median).yellow(),
            format_number(info.stddev).magenta(),
        )
        .unwrap();
    }

    fn render_temporal_axis(
        &mut self,
        left_padding: usize,
        width: usize,
        ticks: &[(usize, String)],
    ) {
        self.draw_buffer.push('\n');

        let mut buffer = Buffer::empty(Rect {
            x: 0,
            y: 0,
            width: width as u16,
            height: 2,
        });

        buffer.set_string(0, 0, "┌", Style::default());
        buffer.set_string(1, 0, "─".repeat(width - 2), Style::default());
        buffer.set_string((width - 1) as u16, 0, "┐", Style::default());

        let first = &ticks.first().unwrap().1;
        let last = &ticks.last().unwrap().1;

        buffer.set_string(0, 1, first, Style::default());
        buffer.set_string((width - last.width()) as u16, 1, last, Style::default());

        for (i, tick) in ticks.iter().skip(1).take(ticks.len() - 2) {
            buffer.set_string(*i as u16, 0, "┬", Style::default());
            buffer.set_string((i - tick.width() / 2) as u16, 1, tick, Style::default());
        }

        for y in 0..=1 {
            self.pad_left(left_padding);

            for x in 0..(width as u16) {
                write!(
                    &mut self.draw_buffer,
                    "{}",
                    buffer.cell((x, y)).unwrap().symbol()
                )
                .unwrap();
            }

            self.draw_buffer.push('\n');
        }
    }
}

impl Display for SparklineRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.draw_buffer)
    }
}

struct ColorMap {
    map: IndexMap<Vec<u8>, usize>,
}

impl ColorMap {
    fn new() -> Self {
        Self {
            map: new_index_map(),
        }
    }

    fn register(&mut self, name: &[u8]) -> usize {
        let i = self.map.len();

        *self.map.entry(name.to_vec()).or_insert(i)
    }

    fn iter(&self) -> impl Iterator<Item = (usize, &[u8])> {
        self.map
            .iter()
            .map(|(name, category)| (*category, name.as_ref()))
    }

    fn new_mask(&self) -> Vec<bool> {
        vec![false; self.map.len()]
    }
}

struct DistributionInfo {
    count: usize,
    central_tendency_indices: (usize, usize, usize, usize),
    mean: f64,
    median: f64,
    stddev: f64,
    min: f64,
    max: f64,
    max_bin_count: u64,
    bins: usize,
}

#[derive(Debug)]
pub struct Series {
    extent_builder: ExtentBuilder<f64>,
    pub numbers: Vec<f64>,
    categories: Vec<usize>,
    times: Vec<f64>,
}

impl Series {
    #[inline]
    fn new() -> Self {
        Self {
            extent_builder: ExtentBuilder::new(),
            numbers: Vec::new(),
            categories: Vec::new(),
            times: Vec::new(),
        }
    }

    pub fn temporal(capacity: usize) -> Self {
        Self {
            extent_builder: ExtentBuilder::new(),
            numbers: vec![1.0; capacity],
            categories: Vec::new(),
            times: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        Self {
            extent_builder: ExtentBuilder::new(),
            numbers: Vec::with_capacity(capacity),
            categories: Vec::new(),
            times: Vec::new(),
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.numbers.len()
    }

    #[inline]
    fn push(&mut self, x: f64) {
        if self.extent_builder.process(x) {
            self.numbers.push(x);
        }
    }

    #[inline]
    fn try_push_float(&mut self, scale_type: ScaleType, x: f64) -> CliResult<()> {
        if x != 0.0 && !scale_type.accepts(x) {
            Err(format!(
                "given --scale encountered an illegal value ({})!",
                x
            ))?;
        }

        self.push(x);
        Ok(())
    }

    #[inline]
    fn try_push_cell(&mut self, scale_type: ScaleType, cell: &[u8]) -> CliResult<()> {
        let x = fast_float::parse(cell)?;
        self.try_push_float(scale_type, x)
    }

    #[inline]
    fn push_category(&mut self, category: usize) {
        self.categories.push(category);
    }

    #[inline]
    pub fn push_time(&mut self, seconds: f64) {
        self.times.push(seconds);
    }

    fn distribution(&mut self, bins: usize) -> DistributionInfo {
        let extent = self.extent_builder.build().unwrap();

        let mut welford = Welford::new();
        let mut histogram = HistogramBuilder::new(bins, extent);

        for x in self.numbers.iter().copied() {
            histogram.add(x);
            welford.add(x);
        }

        let median = linear_time_median(&mut self.numbers);

        self.extent_builder.clear();
        self.extent_builder.process(0.0);
        self.extent_builder.process(histogram.max_value());

        let mean = welford.mean().unwrap();
        let stddev = welford.stddev().unwrap();

        let indices = (
            histogram.discrete_index(mean),
            histogram.discrete_index(median),
            histogram.discrete_index(mean - stddev),
            histogram.discrete_index(mean + stddev),
        );

        let info = DistributionInfo {
            min: extent.min(),
            max: extent.max(),
            central_tendency_indices: indices,
            count: self.numbers.len(),
            mean,
            median,
            stddev,
            bins,
            max_bin_count: histogram.max_count(),
        };

        self.numbers = histogram.into_vec();

        info
    }

    pub fn discretize(&mut self, count: usize) {
        debug_assert!(count < self.numbers.len());

        self.extent_builder.clear();

        let mut bins = Vec::with_capacity(count);
        let chunk_size = (self.numbers.len() as f64 / count as f64).ceil() as usize;

        for chunk in self.numbers.chunks(chunk_size) {
            let sum = chunk.iter().copied().sum();
            bins.push(sum);
            self.extent_builder.process(sum);
        }

        self.numbers = bins;

        if !self.categories.is_empty() {
            let mut categories_bins = Vec::with_capacity(count);

            for chunk in self.categories.chunks_mut(chunk_size) {
                chunk.sort();
                let mode = chunk[0];
                // NOTE: in case of ties we should probably keep the first seen in original sequence
                // but maybe it is good enough as-is.
                categories_bins.push(mode);
            }

            self.categories = categories_bins;
        }
    }

    fn categorical_sort(&mut self, color_map: &ColorMap) {
        debug_assert!(self.categories.len() == self.numbers.len());

        let mut category_mask = color_map.new_mask();

        for category in self.categories.iter().copied() {
            category_mask[category] = true;
        }

        for (category, mask) in category_mask.iter().copied().enumerate() {
            if !mask {
                self.push(0.0);
                self.categories.push(category);
            }
        }

        let mut indices = (0..self.numbers.len()).collect::<Vec<_>>();
        indices.sort_by(|a, b| self.categories[*a].cmp(&self.categories[*b]));

        let mut new_numbers = Vec::with_capacity(self.numbers.len());
        let mut new_categories = Vec::with_capacity(self.categories.len());

        for i in indices {
            new_numbers.push(self.numbers[i]);
            new_categories.push(self.categories[i]);
        }

        self.numbers = new_numbers;
        self.categories = new_categories;
    }

    pub fn temporal_discretize_and_sort(
        &mut self,
        count: usize,
        unit: Unit,
        extent: &TemporalExtent,
        aggregation: Aggregation,
    ) -> CliResult<()> {
        debug_assert!(self.times.len() == self.numbers.len());

        let max_gap = count / 20;

        let has_categories = !self.categories.is_empty();

        debug_assert!(if has_categories {
            self.numbers.len() == self.categories.len()
        } else {
            true
        });

        let seconds_extent = build_seconds_extent(extent, unit)?;

        let mut new_numbers: Vec<_> = (0..count).map(|_| aggregation.new_aggregator()).collect();
        let mut new_categories: Vec<Vec<(usize, usize)>> = (0..count).map(|_| Vec::new()).collect();

        for (original_index, original_seconds) in self.times.iter().copied().enumerate() {
            let new_seconds = Timestamp::from_secs_f64(original_seconds)?
                .to_zoned(TimeZone::system())
                .floor(unit)?
                .timestamp()
                .as_duration()
                .as_secs_f64();

            let new_index = seconds_extent.discrete_index(count, new_seconds);

            new_numbers[new_index].add(self.numbers[original_index]);

            if has_categories {
                let original_category = self.categories[original_index];

                if let Some(count) = new_categories[new_index].iter_mut().find_map(|(c, count)| {
                    if *c == original_category {
                        Some(count)
                    } else {
                        None
                    }
                }) {
                    *count += 1;
                } else {
                    new_categories[new_index].push((original_category, 1));
                }
            }
        }

        self.extent_builder.clear();

        let mut new_numbers: Vec<_> = new_numbers
            .into_iter()
            .map(|aggregator| aggregator.get())
            .collect();

        fill_discretization_gaps(&mut new_numbers, max_gap, |left, right, t| {
            left + t * (right - left)
        });

        self.numbers = new_numbers
            .into_iter()
            .map(|x_opt| {
                let x = x_opt.unwrap_or(0.0);

                self.extent_builder.process(x);

                x
            })
            .collect();

        if has_categories {
            let mut new_categories: Vec<_> = new_categories
                .into_iter()
                .map(|candidates| {
                    candidates
                        .iter()
                        .max_by(|a, b| a.1.cmp(&b.1))
                        .map(|(c, _)| *c)
                })
                .collect();

            fill_discretization_gaps(&mut new_categories, max_gap, |left, _, _| left);

            self.categories = new_categories
                .into_iter()
                .map(|c_opt| c_opt.unwrap_or(usize::MAX))
                .collect()
        }

        Ok(())
    }

    pub fn to_scale(&self, scale_type: ScaleType) -> Option<Scale> {
        self.extent_builder.build().map(|mut extent| {
            if extent.min() == 0.0 && scale_type.disallows_zero() {
                extent.set_min(1.0);
            }

            Scale::from_extent(scale_type, extent)
        })
    }
}

static USAGE: &str = r#"
Print ASCII sparklines (using ▁▂▃▄▅▆▇ characters) from CSV data.

This command is able to represent arbitrary numerical series as well as
categorical data, time series, distributions etc.

Print two numerical columns:

    $ xan spark count1,count2 data.csv

    count1 ▅▆▅▇▅▃▅▅▁▅▅▄▅▄▆▃▅▄▃▁
    count2 ▆▆▅▇▅▃▅▅▁▆▆▄▅▄▅▃▄▅▄▁

Print the distribution of two numerical columns:

    $ xan spark count1,count2 -D -H 2 -z data.csv

           ▇▃
    count1 ██▇▆▆▂▁▃▁▁ ▁▂▃▁▃▁▂
           ▇▂
    count2 ██▇▆▆▆▆▁▄ ▂ ▂▃▂▂▁▂

Print time series grouped by the value of a column:

    $ xan spark value -T date -g group data.csv

    group1    ▁▂▃▄▆▇▇▅▃▂▁▁▁
    group2           ▂▇▇▇▄▁
    group3 ▁▂▃▇▆▆▆▄▂▁▁▁
    group4      ▂▄▅▇▄▆▃▂▂▂▁
    group5           ▁▂▂▄▇▄
    group6 ▅▇▇▆▃▁▁▁▁▁▁▁▁▁▂▁

Print a vertical bar chart from the output of `xan freq`:

    $ xan freq -s category data.csv | xan spark count -c value -H .5 -W7 -C always --hide-names -N

    ▇▇▇▇▇▇▃▃▃▃▃▃
    ████████████▆▆▆▆▆▆▂▂▂▂▂▂▁▁▁▁▁▁
    ██████████████████████████████▁▁▁▁▁▁
    94    85    75    66    64    48
    Vinyl  Disc Other Downl… Tape Strea…

Print categorical bar chart by group:

    $ xan spark value -c category -g author_name

    group1 ▇▁▁
    group1  ▇
    group1    ▇
    group1 ▂  ▇
    group1 ▇ ▁

Print the very literal Joy Division plot from the "Unknown Pleasures" album cover:

    $ curl https://gist.githubusercontent.com/borgar/31c1e476b8e92a11d7e9/raw/0fae97dab6830ecee185a63c1cee0008f6778ff6/pulsar.csv | \
    $ xan spark --along-rows '*' --hide-all

Unzoom your terminal for better effect ;)

Usage:
    xan spark debate
    xan spark --count [options] [<input>]
    xan spark [options] [--] <y> [<input>]
    xan spark --help

spark mode options:
    --along-rows          Collect series to print along rows, instead of along
                          columns.
    -T, --time <col>      Use selected <col> to position points in time and
                          reorder x axis chronologically.
    -D, --dist            Reinterpret given series by printing an histogram
                          of their distribution instead.
    -g, --groupby <cols>  Print one series per value found in <cols> selection.
    -c, --category <col>  Choose a <col> to represent a data point's category
                          in printed series. Will be used to select a color
                          for the data point and is therefore incompatible with
                          the other coloring flags below.

spark options:
    -W, --width <n>     Number of characters wide a sparkline bar is allowed to be.
                        [default: 1]
    -H, --height <n>    Number of characters high a sparkline bar is allowed to be.
                        Can also be given as a ratio or percentage of the terminal's
                        height e.g. "45%" or "0.5". Defaults to 1.
    --scale <scale>     Apply a scale to the y axis. Can be one of "lin", "pow",
                        "sqrt", "pow(custom_exponent)" like "pow(4.5)", "log",
                        "log2", "log10" or "log(custom_base)" like "log(2.5)".
                        [default: lin]
    --log               Use a log scale, shorthand for --scale=log.
    -m, --min <n>       Force <y> minimum value. Any value falling out of range will be
                        filtered out.
    -M, --max <n>       Force <y> maximum value. Any value falling out of range will be
                        filtered out.
    --share-scale       Whether to force series to share their y-axis.
    --hide-names        Whether to hide series' names.
    --hide-legend       Whether to hide any kind of legend.
    --hide-all          Shorthand for --hide-names, --hide-legend.
    -F, --flatter       Print series names on top of them instead of to their left, to
                        make more space for series themselves.
    -w, --wrap          Allow series to overflow on muliple lines instead of discretizing
                        them to fit your terminal's width.
    -S, --small-multiples <n>
                        When used, will display <n> series per row instead of a single one.
                        This is useful to see more series at once if you have enough space.
    -N, --show-numbers  Show series numbers under their respective bars. Only useful
                        when -W/--width is more than 1.
    -P, --show-percentages
                        Show series numbers as a percentage under their respective bars. Only
                        useful when -W/--width is more than 1.
    --cols <num>        Number of terminal columns, i.e. characters, that we can
                        use for drawing labels, legends and sparklines.
                        Defaults to using all your terminal's width or 80 if
                        terminal size cannot be found (i.e. when piping to file).
                        Can also be given as a ratio or percentage of the terminal's width
                        e.g. "45%" or "0.5".
    --color <when>      When to color the output using ANSI escape codes.
                        Use `auto` for automatic detection, `never` to
                        disable colors completely and `always` to force
                        colors, even when the output could not handle them.
                        [default: auto]

spark coloring options:
    -G, --gradient <name>             Color each bar using given gradient.
    -B, --background-gradient <name>  Hide bars and print a background color using
                                      given gradient. The result can be thought of as a kind
                                      of heatmap.
    -V, --vertical-gradient <name>    Color bars with given gradient but map the color
                                      on a character's height in a bar. Use this for
                                      aesthetic purposes only. This is only ever useful
                                      when -H/--height is more than 1.
    -R, --rainbow                     Assign a color to each series following a cyclical pattern.
                                      This is useful to distinguish between series when using
                                      multiple <y> columns or -g/--groupby.
    -z, --striped                     Dim each odd bar's color for better readability.

See `xan help gradients` for a list of available gradients.

spark -T/--time options:
    --count                 Count rows falling into a same temporal bucket instead of
                            relying on a numerical column.
    --sort                  Sort given time series by starting point.
    -A, --aggregate <mode>  How to aggregate values falling into a same bucket when discretizing
                            a temporal x axis, e.g. when using the -T/--time flag.
                            Can be one of "sum" or "mean". Defaults to "sum" when --count
                            is given, else "mean".

spark -D/--dist options:
    -b, --bins <n>  Number of bins for the distribution histogram. [default: 35]

spark -c/--category options:
    -C, --cram <choice>  When printing a categorical legend, whether to attempt
                         cramming category names under a series' bars. Can be
                         either "always", "never" or "auto". When "auto" is given,
                         names will be crammed if theire is enough place for them.
                         This is really only useful when -W/--width is more than 1.
                         [default: auto]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
"#;

#[derive(Deserialize, Debug)]
struct Args {
    cmd_debate: bool,
    arg_input: Option<String>,
    arg_y: Option<SelectedColumns>,
    flag_groupby: Option<SelectedColumns>,
    flag_category: Option<SelectedColumns>,
    flag_along_rows: bool,
    flag_gradient: Option<GradientName>,
    flag_background_gradient: Option<GradientName>,
    flag_vertical_gradient: Option<GradientName>,
    flag_small_multiples: Option<NonZeroUsize>,
    flag_hide_names: bool,
    flag_hide_legend: bool,
    flag_hide_all: bool,
    flag_time: Option<SelectedColumns>,
    flag_aggregate: Option<Aggregation>,
    flag_count: bool,
    flag_share_scale: bool,
    flag_striped: bool,
    flag_rainbow: bool,
    flag_bins: NonZeroUsize,
    flag_dist: bool,
    flag_log: bool,
    flag_sort: bool,
    flag_cram: CramMode,
    flag_scale: ScaleType,
    flag_min: Option<f64>,
    flag_max: Option<f64>,
    flag_wrap: bool,
    flag_show_numbers: bool,
    flag_show_percentages: bool,
    flag_flatter: bool,
    flag_width: NonZeroUsize,
    flag_height: Option<String>,
    flag_cols: Option<String>,
    flag_color: ColorMode,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_hide_all {
            self.flag_hide_names = true;
            self.flag_hide_legend = true;
        }
    }

    fn new_series(&self, capacity_opt: Option<usize>) -> Series {
        let mut series = if let Some(capacity) = capacity_opt {
            Series::with_capacity(capacity)
        } else {
            Series::new()
        };

        if let Some(min) = self.flag_min {
            series.extent_builder.clamp_min(min);
        }

        if let Some(max) = self.flag_max {
            series.extent_builder.clamp_max(max);
        }

        series
    }

    fn aggregation(&self) -> Aggregation {
        self.flag_aggregate.unwrap_or(if self.flag_count {
            Aggregation::Sum
        } else {
            Aggregation::Mean
        })
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    if args.cmd_debate {
        eprintln!(
            "✨💖✨ I love CSV! ✨💖✨\nhttps://github.com/medialab/xan/blob/master/docs/LOVE_LETTER.md"
        );
        return Ok(());
    }

    let color_mode = args.flag_category.is_some() as u8
        + args.flag_gradient.is_some() as u8
        + args.flag_background_gradient.is_some() as u8
        + args.flag_vertical_gradient.is_some() as u8
        + args.flag_rainbow as u8;

    if color_mode > 1 {
        Err(
            "only one of -c/--category, -R/--rainbow, -G/--gradient, -V/--vertical-gradient or -B/--background-gradient can be used at once!",
        )?;
    }

    if args.flag_striped
        && (args.flag_category.is_some()
            || args.flag_gradient.is_some()
            || args.flag_background_gradient.is_some()
            || args.flag_vertical_gradient.is_some())
    {
        Err(
            "-z/--striped does not work with -c/--category, -G/--gradient, -V/--vertical-gradient nor -B/--background-gradient!",
        )?;
    }

    if args.flag_along_rows {
        if args.flag_groupby.is_some() {
            Err("-g/--groupby does not work with --along-rows!")?;
        }

        if args.flag_category.is_some() {
            Err("-c/--category does not work with --along-rows!")?;
        }

        if args.flag_time.is_some() {
            Err("-T/--time does not work with --along-rows!")?;
        }
    }

    if args.flag_dist && (args.flag_category.is_some() || args.flag_time.is_some()) {
        Err("-D/--dist does not work with -c/--category nor -T/--time!")?;
    }

    if args.flag_wrap && args.flag_small_multiples.is_some() {
        Err("-w/--wrap does not work with -S/--small-multiples")?;
    }

    if args.flag_count && args.flag_time.is_none() {
        Err("--count can only be used with -T/--time!")?;
    }

    if args.flag_flatter {
        if args.flag_hide_names {
            Err("-F/--flatter does not make sense with --hide-names!")?;
        }

        if args.flag_small_multiples.is_some() {
            Err("-F/--flatter does not work with -S/--small-multiples!")?;
        }
    }

    if args.flag_sort && args.flag_time.is_none() {
        Err("--sort only works with -T/--time!")?;
    }

    if args.flag_log {
        args.flag_scale = ScaleType::ln();
    }

    if matches!(args.flag_min, Some(v) if !args.flag_scale.accepts(v))
        || matches!(args.flag_max, Some(v) if !args.flag_scale.accepts(v))
    {
        Err("-m/--min or -M/--max values are incompatible with --scale!")?;
    }

    args.flag_color.apply();

    let mut cols = util::acquire_term_cols_ratio(&args.flag_cols)?;

    if cols < 10 {
        Err("not enough cols to draw!")?;
    }

    let mut out = stdout();

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut reader = rconf.simd_reader()?;
    let headers = reader.byte_headers()?.clone();

    let sel_opt = args
        .arg_y
        .as_ref()
        .map(|s| s.selection(&headers, !rconf.no_headers))
        .transpose()?;

    let groupby_opt: Option<(Selection, ClusteredInsertHashmap<ByteRecord, Series>)> = args
        .flag_groupby
        .as_ref()
        .map(|s| s.selection(&headers, !rconf.no_headers))
        .transpose()?
        .map(|s| (s, ClusteredInsertHashmap::new()));

    if groupby_opt.is_some() && sel_opt.as_ref().map(|s| s.len()).unwrap_or(1) > 1 {
        Err("only one value column must be selected when using -g/--groupby!")?;
    }

    let selected_column_names = sel_opt.as_ref().map(|sel| {
        sel.select(&headers)
            .map(|h| String::from_utf8_lossy(h).into_owned())
            .collect::<Vec<_>>()
    });

    let group_column_names = groupby_opt.as_ref().map(|(sel, _)| {
        sel.select(&headers)
            .map(|h| String::from_utf8_lossy(h).into_owned())
            .collect::<Vec<_>>()
    });

    let mut categories_opt = args
        .flag_category
        .as_ref()
        .map(|s| s.single_selection(&headers, !rconf.no_headers))
        .transpose()?
        .map(|i| (i, ColorMap::new()));

    let name_hash = if let Some((category_column_index, _)) = categories_opt.as_ref() {
        compute_name_hash(&headers[*category_column_index])
    } else {
        0
    };

    let mut time_opt: Option<(usize, TemporalExtent)> = args
        .flag_time
        .as_ref()
        .map(|s| s.single_selection(&headers, !rconf.no_headers))
        .transpose()?
        .map(|i| (i, TemporalExtent::new()));

    let mut record = ByteRecord::new();

    let mut pool: Vec<(String, Series)> = Vec::new();

    // Aggregating data
    if let Some((groupby_sel, mut series_map)) = groupby_opt {
        while reader.read_byte_record(&mut record)? {
            let group = groupby_sel.select(&record).collect();

            let series = series_map.insert_with(group, || args.new_series(None));

            if let Some(sel) = &sel_opt {
                series.try_push_cell(args.flag_scale, &record[sel[0]])?;
            } else {
                debug_assert!(args.flag_count);
                series.try_push_float(args.flag_scale, 1.0)?;
            }

            if let Some((category_column_index, color_map)) = categories_opt.as_mut() {
                let category = color_map.register(&record[*category_column_index]);
                series.push_category(category);
            }

            if let Some((time_column_index, extent)) = time_opt.as_mut() {
                let (t, seconds) = parse_temporal(&record[*time_column_index])?;
                extent.add(t.into())?;
                series.push_time(seconds);
            }
        }

        for (group, mut series) in series_map.into_iter() {
            let name = group
                .iter()
                .map(|cell| String::from_utf8_lossy(cell).into_owned())
                .collect::<Vec<_>>()
                .join(", ");

            if let Some((_, color_map)) = categories_opt.as_ref() {
                if time_opt.is_none() {
                    series.categorical_sort(color_map);
                }
            }

            pool.push((name, series));
        }
    } else {
        if !args.flag_along_rows {
            if let Some(sel) = &sel_opt {
                pool.reserve_exact(sel.len());

                for name in sel.select(&headers) {
                    pool.push((
                        String::from_utf8_lossy(name).into_owned(),
                        args.new_series(None),
                    ));
                }
            } else {
                pool.push(("--count".to_string(), args.new_series(None)));
            }
        }

        let mut index: usize = 0;

        while reader.read_byte_record(&mut record)? {
            if args.flag_along_rows {
                let sel = sel_opt.as_ref().unwrap();
                let mut series = args.new_series(Some(sel.len()));

                for cell in sel.select(&record) {
                    series.try_push_cell(args.flag_scale, cell)?;
                }

                pool.push((format!("Row n°{}", index), series));
            } else {
                let category_opt =
                    if let Some((category_column_index, color_map)) = categories_opt.as_mut() {
                        Some(color_map.register(&record[*category_column_index]))
                    } else {
                        None
                    };

                let seconds_opt = if let Some((time_column_index, extent)) = time_opt.as_mut() {
                    let (t, seconds) = parse_temporal(&record[*time_column_index])?;
                    extent.add(t.into())?;
                    Some(seconds)
                } else {
                    None
                };

                if let Some(sel) = &sel_opt {
                    for (i, cell) in sel.select(&record).enumerate() {
                        let series = &mut pool[i].1;

                        series.try_push_cell(args.flag_scale, cell)?;

                        if let Some(category) = category_opt {
                            series.push_category(category);
                        }

                        if let Some(seconds) = seconds_opt {
                            series.push_time(seconds);
                        }
                    }
                } else {
                    debug_assert!(args.flag_count);
                    let series = &mut pool[0].1;

                    series.try_push_float(args.flag_scale, 1.0)?;

                    if let Some(category) = category_opt {
                        series.push_category(category);
                    }

                    if let Some(seconds) = seconds_opt {
                        series.push_time(seconds);
                    }
                }
            }

            index += 1;
        }
    }

    let full_cols = cols;

    // Layout
    if let Some(small_multiples) = args.flag_small_multiples {
        let n = small_multiples.get();

        if n < 2 {
            Err("-S/--small-multiples cannot be less than 2!")?;
        }

        cols -= n - 1;
        cols /= n;
    }

    let mut cols_for_sparkline = cols;
    let sparkline_width = args.flag_width.get();

    let rows = util::acquire_term_rows_ratio(&args.flag_height)?;

    let sparkline_height = if args.flag_height.is_some() { rows } else { 1 };

    let mut cols_for_series_name: usize = 0;

    let max_name_width = pool.iter().map(|(name, _)| name.width()).max().unwrap() + 1;

    if !args.flag_hide_names {
        cols_for_series_name = max_name_width.min((cols as f64 * 0.3).floor() as usize);
        cols_for_sparkline -= cols_for_series_name;
    }

    let max_bins = cols_for_sparkline / sparkline_width;

    // Recasting as distribution
    let distribution_infos_opt = if args.flag_dist {
        let mut infos = Vec::with_capacity(pool.len());

        for (_, series) in pool.iter_mut() {
            infos.push(series.distribution(args.flag_bins.get()));
        }

        Some(infos)
    } else {
        None
    };

    // Temporal discretization
    let mut granularity_opt = None;

    if let Some((_, extent)) = &time_opt {
        let (adjusted_bins, best_unit) = extent.best_discrete_granularity(max_bins)?.unwrap();

        granularity_opt = Some(best_unit);

        for (_, series) in pool.iter_mut() {
            series.temporal_discretize_and_sort(
                adjusted_bins,
                best_unit,
                extent,
                args.aggregation(),
            )?;
        }

        if args.flag_sort {
            pool.sort_by_key(|(_, series)| {
                series
                    .times
                    .iter()
                    .map(|f| NotNan::new(*f).unwrap())
                    .min()
                    .unwrap()
            });
        }
    }

    // Layout discretization
    if !args.flag_wrap {
        for (_, series) in pool.iter_mut() {
            if series.len() > max_bins {
                series.discretize(max_bins);
            }
        }
    }

    // Temporal x-axis
    let mut temporal_ticks_opt = None;

    if let Some((_, extent)) = &time_opt {
        let discretized_series_len = pool.first().unwrap().1.len();
        let granularity = granularity_opt.unwrap();

        let min_label = extent
            .earliest()
            .unwrap()
            .to_lower_bound_timestamp(TimeZone::system())?
            .to_zoned(TimeZone::system())
            .to_string_wrt_unit(granularity);

        let max_label = extent
            .latest()
            .unwrap()
            .to_lower_bound_timestamp(TimeZone::system())?
            .to_zoned(TimeZone::system())
            .to_string_wrt_unit(granularity);

        let budget_cols = discretized_series_len
            .saturating_sub(min_label.width())
            .saturating_sub(max_label.width());

        if budget_cols > 0 {
            let baseline = min_label.width();
            let mut ticks = vec![(0, min_label)];

            let additional_ticks =
                budget_cols.saturating_sub(2).saturating_sub(baseline * 2) / (baseline + 2);

            if additional_ticks > 0 {
                let seconds_extent = build_seconds_extent(extent, granularity)?;

                fn lerp(min: f64, max: f64, t: f64) -> f64 {
                    (1.0 - t) * min + t * max
                }

                let mut t = 0.0;
                let fract = 1.0 / (additional_ticks + 1) as f64;

                for _ in 0..additional_ticks {
                    t += fract;
                    let i = lerp(0.0, discretized_series_len as f64, t) as usize;
                    let seconds = lerp(seconds_extent.min(), seconds_extent.max(), t);
                    let tick = Timestamp::from_secs_f64(seconds)?
                        .to_zoned(TimeZone::system())
                        .to_string_wrt_unit(granularity);

                    ticks.push((i, tick));
                }
            }

            ticks.push((discretized_series_len - 1, max_label));

            temporal_ticks_opt = Some(ticks);
        }
    }

    // Scale sharing
    if pool.len() > 1 && args.flag_share_scale {
        let mut total_extent = ExtentBuilder::new();

        for (_, series) in pool.iter() {
            total_extent.merge(&series.extent_builder);
        }

        for (_, series) in pool.iter_mut() {
            series.extent_builder = total_extent.clone();
        }
    }

    // Padding
    if args.flag_groupby.is_some()
        && args.flag_small_multiples.is_some()
        && categories_opt.is_none()
        && time_opt.is_none()
    {
        let longest = pool.iter().map(|(_, series)| series.len()).max().unwrap();

        for (_, series) in pool.iter_mut() {
            while series.len() < longest {
                series.push(0.0);
            }
        }
    }

    // Re-adjusting `cols_for_series_name`
    let max_sparkline_width = pool
        .iter()
        .map(|(_, series)| series.len() * sparkline_width)
        .max()
        .unwrap();

    if !args.flag_hide_names
        && max_sparkline_width < cols_for_sparkline
        && max_name_width > cols_for_series_name
    {
        let diff = cols_for_sparkline - max_sparkline_width;
        cols_for_series_name += diff;

        if cols_for_series_name > max_name_width {
            cols_for_series_name = max_name_width;
        }
    }

    let name_padding = " ".repeat(cols_for_series_name);

    // Building renderer
    let mut sparkline_renderer_options = SparklineRendererOptions::new();
    sparkline_renderer_options.width = sparkline_width;
    sparkline_renderer_options.height = sparkline_height;

    if let Some(gradient) = args.flag_gradient {
        sparkline_renderer_options.color_mode =
            SparklineColorMode::Gradient(gradient.build(), false);
    } else if let Some(gradient) = args.flag_vertical_gradient {
        sparkline_renderer_options.color_mode =
            SparklineColorMode::Gradient(gradient.build(), true);
    } else if let Some(gradient) = args.flag_background_gradient {
        sparkline_renderer_options.color_mode =
            SparklineColorMode::BackgroundGradient(gradient.build());
    } else if args.flag_striped {
        if args.flag_rainbow {
            sparkline_renderer_options.color_mode = SparklineColorMode::StripedRainbow;
        } else {
            sparkline_renderer_options.color_mode = SparklineColorMode::Striped;
        }
    } else if args.flag_rainbow {
        sparkline_renderer_options.color_mode = SparklineColorMode::Rainbow;
    }

    let mut sparkline_renderer = sparkline_renderer_options.build();

    let mut small_multiples_buffer_opt: Option<Vec<String>> =
        args.flag_small_multiples.map(|_| Vec::new());

    let mut colors_buffer: Vec<Option<ColorOrStyles>> = Vec::new();

    let actually_cram = match args.flag_cram {
        CramMode::Always => categories_opt.is_some(),
        CramMode::Never => false,
        CramMode::Auto => categories_opt
            .as_ref()
            .map(|(_, color_map)| {
                color_map
                    .iter()
                    .all(|(_, name)| name.len() <= args.flag_width.get())
            })
            .unwrap_or(false),
    };

    if !args.flag_hide_legend {
        writeln!(
            &mut out,
            "\nDisplaying {}{} of {}{}{}\n",
            if args.flag_dist {
                "distribution"
            } else if args.flag_along_rows {
                "row-wise series"
            } else if args.flag_time.is_some() {
                "time series"
            } else {
                "column-wise series"
            },
            if let Some((_, extent)) = &time_opt {
                let granularity = granularity_opt.unwrap();

                format!(
                    " (from {} to {}, by {})",
                    extent.earliest().unwrap().to_string().cyan(),
                    extent.latest().unwrap().to_string().cyan(),
                    Granularity::new(granularity).to_string().green()
                )
            } else {
                "".to_string()
            },
            if let Some(names) = selected_column_names {
                if args.flag_along_rows {
                    format!(
                        "from {} to {}",
                        names.first().unwrap().green(),
                        names.last().unwrap().green()
                    )
                } else {
                    names
                        .into_iter()
                        .map(|n| n.green().to_string())
                        .collect::<Vec<_>>()
                        .join(" & ")
                }
            } else {
                "--count".dimmed().to_string()
            },
            if let Some(names) = group_column_names {
                format!(
                    ", broken down by {}",
                    names
                        .into_iter()
                        .map(|n| n.green().to_string())
                        .collect::<Vec<_>>()
                        .join(" & ")
                )
            } else {
                "".to_string()
            },
            if args.flag_scale.is_linear() {
                "".to_string()
            } else {
                format!(", {} scale", args.flag_scale.to_string().cyan())
            }
        )?;

        // Categorical legend
        if !actually_cram {
            if let Some((_, color_map)) = &categories_opt {
                writeln!(&mut out, "Categories:")?;

                for (category, name) in color_map.iter() {
                    let color =
                        util::colorizer_by_rainbow_with_fallback(category, name_hash, "spark");

                    writeln!(
                        &mut out,
                        "{} {}",
                        color.colorize("■"),
                        util::unicode_aware_ellipsis(
                            &util::sanitize_text_for_single_line_printing(
                                &String::from_utf8_lossy(name)
                            ),
                            full_cols.saturating_sub(2)
                        )
                    )?;
                }

                writeln!(&mut out)?;
            }
        }
    }

    // Rendering
    for (i, (name, series)) in pool.into_iter().enumerate() {
        let mut name_opt = (!args.flag_hide_names).then(|| {
            format!(
                "{:<width$} ",
                util::unicode_aware_ellipsis(
                    &util::sanitize_text_for_multi_line_printing(&name),
                    cols_for_series_name.saturating_sub(1),
                ),
                width = cols_for_series_name.saturating_sub(1)
            )
        });

        if args.flag_flatter {
            writeln!(&mut out, "{}", name_opt.take().unwrap())?;
        }

        if categories_opt.is_some() {
            colors_buffer.clear();

            for category in series.categories.iter().copied() {
                colors_buffer.push(Some(util::colorizer_by_rainbow_with_fallback(
                    category, name_hash, "spark",
                )));
            }
        }

        let scale = series.to_scale(args.flag_scale).unwrap();

        let chunk_size = if args.flag_wrap {
            max_bins
        } else {
            series.len()
        };

        let series_sum_opt = args
            .flag_show_percentages
            .then(|| series.numbers.iter().sum::<f64>());

        let mut offset: usize = 0;

        for (chunk_i, chunk) in series.numbers.chunks(chunk_size).enumerate() {
            sparkline_renderer.render_impl(
                i,
                if chunk_i == 0 {
                    name_opt.as_deref()
                } else if name_opt.is_some() {
                    Some(name_padding.as_str())
                } else {
                    None
                },
                &scale,
                chunk,
                if categories_opt.is_some() {
                    Some(&colors_buffer[offset..offset + chunk.len()])
                } else {
                    None
                },
            );

            if !args.flag_hide_legend {
                if let Some(central_tendencies) = &distribution_infos_opt {
                    let info = central_tendencies.get(i).unwrap();

                    sparkline_renderer.render_distribution_legend(name_padding.len(), info);
                }

                if args.flag_show_percentages {
                    sparkline_renderer.show_numbers(name_padding.len(), chunk, series_sum_opt);
                }

                if args.flag_show_numbers {
                    sparkline_renderer.show_numbers(name_padding.len(), chunk, None);
                }

                if actually_cram {
                    let color_map = &categories_opt.as_ref().unwrap().1;

                    sparkline_renderer
                        .cram_names(name_padding.len(), color_map.iter().map(|(_, name)| name));
                }

                if let Some(ticks) = &temporal_ticks_opt {
                    sparkline_renderer.render_temporal_axis(name_padding.len(), chunk.len(), ticks);
                }
            }

            if let Some(small_multiples_buffer) = small_multiples_buffer_opt.as_mut() {
                let sparkline = sparkline_renderer.to_string();

                small_multiples_buffer.push(sparkline);
            } else {
                writeln!(&mut out, "{}", sparkline_renderer)?;
            }

            offset += chunk.len();
        }

        if args.flag_flatter {
            writeln!(&mut out)?;
        }
    }

    // Rendering small multiples
    if let Some(small_multiples_buffer) = small_multiples_buffer_opt {
        let mut output_buffer = String::new();

        for row in small_multiples_buffer.chunks(args.flag_small_multiples.unwrap().get()) {
            let mut row_lines = row
                .iter()
                .map(|sparkline| sparkline.split('\n'))
                .collect::<Vec<_>>();

            while let Some(line) = row_lines[0].next() {
                output_buffer.push_str(line);

                for line_iter in row_lines[1..].iter_mut() {
                    output_buffer.push(' ');
                    output_buffer.push_str(line_iter.next().unwrap());
                }

                output_buffer.push('\n');
            }
        }

        write!(&mut out, "{}", output_buffer)?;
    }

    Ok(())
}
