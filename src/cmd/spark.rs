use std::fmt::{Display, Write as FmtWrite};
use std::io::{stdout, Write};
use std::num::NonZeroUsize;

use colored::Colorize;
use colorgrad::Gradient;
use simd_csv::ByteRecord;
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::scales::{ExtentBuilder, GradientName, Scale, ScaleType};
use crate::select::SelectedColumns;
use crate::util::{self, ColorMode, ColorOrStyles};
use crate::CliResult;

pub static SPARKLINE_CHARS: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];
pub const FULL_BAR: char = '█';

#[derive(Default, Clone)]
enum SparklineColorMode {
    #[default]
    None,
    Striped,
    Gradient(Box<dyn Gradient>),
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
        self.render_impl(None, scale, bins, None);
    }

    #[inline(always)]
    pub fn render_with_color_overrides(
        &mut self,
        scale: &Scale,
        bins: &[f64],
        color_overrides_opt: Option<&[Option<ColorOrStyles>]>,
    ) {
        self.render_impl(None, scale, bins, color_overrides_opt);
    }

    pub fn render_impl(
        &mut self,
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
                let ratio = if y == 0.0 { 0.0 } else { scale.percent(y) };

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
                            SparklineColorMode::Gradient(gradient) => {
                                let c = gradient.at(ratio as f32).to_rgba8();

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

    pub fn render_central_tendency(
        &mut self,
        bins: usize,
        mean_index: usize,
        median_index: usize,
        sigma_left_index: usize,
        sigma_right_index: usize,
    ) {
        self.draw_buffer.push('\n');

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
}

impl Display for SparklineRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.draw_buffer)
    }
}

#[derive(Debug)]
struct Series {
    extent_builder: ExtentBuilder<f64>,
    numbers: Vec<f64>,
}

impl Series {
    #[inline]
    fn new() -> Self {
        Self {
            extent_builder: ExtentBuilder::new(),
            numbers: Vec::new(),
        }
    }

    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        Self {
            extent_builder: ExtentBuilder::new(),
            numbers: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    fn len(&self) -> usize {
        self.numbers.len()
    }

    #[inline]
    fn push(&mut self, x: f64) {
        self.numbers.push(x);
        self.extent_builder.process(x);
    }

    #[inline]
    fn try_push(&mut self, cell: &[u8]) -> CliResult<()> {
        let x = fast_float::parse(cell)?;
        self.push(x);
        Ok(())
    }

    fn discretize(&mut self, count: usize) {
        if count < self.numbers.len() {
            self.extent_builder.clear();

            let mut bins = Vec::with_capacity(count);
            let chunk_size = (self.numbers.len() as f64 / count as f64).ceil() as usize;

            for chunk in self.numbers.chunks(chunk_size) {
                let sum = chunk.iter().copied().sum();
                bins.push(sum);
                self.extent_builder.process(sum);
            }

            self.numbers = bins;
        } else {
            unimplemented!()
        }
    }

    fn to_scale(&self, scale_type: ScaleType) -> Option<Scale> {
        self.extent_builder
            .build()
            .map(|extent| Scale::from_extent(scale_type, extent))
    }
}

static USAGE: &str = "
TODO...

Usage:
    xan spark debate
    xan spark [options] [--] <columns> [<input>]
    xan spark --help

spark options:
    --along-rows
    -W, --width <n>   Number of characters a sparkline bar is allowed to take as
                      its width.
                      [default: 1]
    -H, --height <n>  Number of characters a sparkline bar is allowed to take as
                      its height.
                      [default: 1]
    -G, --gradient <name>
    -B, --background-gradient <name>
    --cols <num>      Number of terminal columns, i.e. characters, that we can
                      use for drawing labels, legends and sparklines.
                      Defaults to using all your terminal's width or 80 if
                      terminal size cannot be found (i.e. when piping to file).
                      Can also be given as a ratio or percentage of the terminal's width
                      e.g. \"45%\" or \"0.5\".
    --color <when>    When to color the output using ANSI escape codes.
                      Use `auto` for automatic detection, `never` to
                      disable colors completely and `always` to force
                      colors, even when the output could not handle them.
                      [default: auto]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    cmd_debate: bool,
    arg_input: Option<String>,
    arg_columns: SelectedColumns,
    flag_along_rows: bool,
    flag_gradient: Option<GradientName>,
    flag_background_gradient: Option<GradientName>,
    flag_width: NonZeroUsize,
    flag_height: NonZeroUsize,
    flag_cols: Option<String>,
    flag_color: ColorMode,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn series_have_name(&self) -> bool {
        !self.flag_along_rows
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_debate {
        eprintln!(
            "✨💖✨ I love CSV! ✨💖✨\nhttps://github.com/medialab/xan/blob/master/docs/LOVE_LETTER.md"
        );
        return Ok(());
    }

    if args.flag_gradient.is_some() && args.flag_background_gradient.is_some() {
        Err("only one of -G/--gradient or -B/--background-gradient can be use at once!")?;
    }

    args.flag_color.apply();

    let cols = util::acquire_term_cols_ratio(&args.flag_cols)?;

    if cols < 10 {
        Err("not enough cols to draw!")?;
    }

    let mut out = stdout();

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .select(args.arg_columns.clone())
        .no_headers(args.flag_no_headers);

    let mut reader = rconf.simd_reader()?;
    let headers = reader.byte_headers()?.clone();
    let sel = rconf.selection(&headers)?;

    let mut record = ByteRecord::new();

    let mut pool: Vec<(Option<String>, Series)> = Vec::new();

    if !args.flag_along_rows {
        pool.reserve_exact(sel.len());

        for name in sel.select(&headers) {
            pool.push((
                Some(String::from_utf8_lossy(name).into_owned()),
                Series::new(),
            ));
        }
    }

    // Aggregating data
    while reader.read_byte_record(&mut record)? {
        if args.flag_along_rows {
            let mut series = Series::with_capacity(sel.len());

            for cell in sel.select(&record) {
                series.try_push(cell)?;
            }

            pool.push((None, series));
        } else {
            for (i, cell) in sel.select(&record).enumerate() {
                pool[i].1.try_push(cell)?;
            }
        }
    }

    let mut cols_for_sparkline = cols;
    let sparkline_width = args.flag_width.get();
    let sparkline_height = args.flag_height.get();

    let mut cols_for_series_name: usize = 0;

    if args.series_have_name() {
        let max_name_width = pool
            .iter()
            .map(|(name_opt, _)| name_opt.as_ref().map(|name| name.width()).unwrap_or(0))
            .max()
            .unwrap()
            + 1;

        cols_for_series_name = max_name_width.min((cols as f64 * 0.3).floor() as usize);
        cols_for_sparkline -= cols_for_series_name;
    }

    let max_bins = cols_for_sparkline / sparkline_width;

    let mut sparkline_renderer_options = SparklineRendererOptions::new();
    sparkline_renderer_options.width = sparkline_width;
    sparkline_renderer_options.height = sparkline_height;

    if let Some(gradient) = args.flag_gradient {
        sparkline_renderer_options.color_mode = SparklineColorMode::Gradient(gradient.build());
    } else if let Some(gradient) = args.flag_background_gradient {
        sparkline_renderer_options.color_mode =
            SparklineColorMode::BackgroundGradient(gradient.build());
    }

    let mut sparkline_renderer = sparkline_renderer_options.build();

    // Rendering
    for (name_opt, mut series) in pool.into_iter() {
        if series.len() > max_bins {
            series.discretize(max_bins);
        }

        let name_opt = name_opt.map(|name| {
            format!(
                "{:<width$} ",
                util::unicode_aware_ellipsis(
                    &util::sanitize_text_for_multi_line_printing(&name),
                    cols_for_series_name.saturating_sub(1),
                ),
                width = cols_for_series_name.saturating_sub(1)
            )
        });

        let scale = series.to_scale(ScaleType::Linear).unwrap();
        sparkline_renderer.render_impl(name_opt.as_deref(), &scale, &series.numbers, None);

        writeln!(&mut out, "{}", sparkline_renderer)?;
    }

    Ok(())
}
