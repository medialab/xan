use std::fmt::{Display, Write as FmtWrite};
use std::io::{stdout, Write};
use std::num::NonZeroUsize;

use colored::Colorize;
use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::scales::{ExtentBuilder, Scale, ScaleType};
use crate::select::SelectedColumns;
use crate::util::{self, ColorMode, ColorOrStyles};
use crate::CliResult;

pub static SPARKLINE_CHARS: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];
pub const FULL_BAR: char = '█';

#[derive(Debug, Default, Clone, Copy)]
enum SparklineColorMode {
    #[default]
    None,
    Striped,
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
        self.render_with_color_overrides(scale, bins, None);
    }

    pub fn render_with_color_overrides(
        &mut self,
        scale: &Scale,
        bins: &[f64],
        color_overrides_opt: Option<&[Option<ColorOrStyles>]>,
    ) {
        let height = self.options.height;
        let width = self.options.width;

        self.draw_buffer.clear();

        for h in (0..height).rev() {
            let len = SPARKLINE_CHARS.len();

            for (i, y) in bins.iter().copied().enumerate() {
                let sparkline_char = if y == 0.0 {
                    ' '
                } else {
                    let ratio = scale.percent(y);
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
                        _ => match self.options.color_mode {
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
                        },
                    };
                }
            }

            self.draw_buffer.push('\n');
        }

        // NOTE: removing last newline
        self.draw_buffer.pop();
    }

    pub fn render_central_tendendy(
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

            let mut bins = vec![0.0; count];
            let chunk_size = (self.numbers.len() as f64 / count as f64).ceil() as usize;

            for (i, chunk) in self.numbers.chunks(chunk_size).enumerate() {
                for x in chunk.iter().copied() {
                    bins[i] += x;
                }

                self.extent_builder.process(bins[i]);
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
    xan spark [options] <columns> [<input>]
    xan spark --help

spark options:
    -W, --width <n>   Number of characters a sparkline bar is allowed to take as
                      its width.
                      [default: 1]
    -H, --height <n>  Number of characters a sparkline bar is allowed to take as
                      its height.
                      [default: 1]
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
    flag_width: NonZeroUsize,
    flag_height: NonZeroUsize,
    flag_cols: Option<String>,
    flag_color: ColorMode,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_debate {
        eprintln!(
            "✨💖✨ I love CSV! ✨💖✨\nhttps://github.com/medialab/xan/blob/master/docs/LOVE_LETTER.md"
        );
        return Ok(());
    }

    args.flag_color.apply();

    let cols = util::acquire_term_cols_ratio(&args.flag_cols)?;

    if cols < 10 {
        Err("not enough cols to draw!")?;
    }

    let mut out = stdout();

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .select(args.arg_columns)
        .no_headers(args.flag_no_headers);

    let mut reader = rconf.simd_reader()?;
    let headers = reader.byte_headers()?.clone();
    let sel = rconf.selection(&headers)?;

    let mut record = ByteRecord::new();

    let mut pool: Vec<Series> = Vec::new();

    // Aggregating data
    while reader.read_byte_record(&mut record)? {
        let mut series = Series::with_capacity(sel.len());

        for cell in sel.select(&record) {
            series.try_push(cell)?;
        }

        pool.push(series);
    }

    let cols_for_sparkline = cols;
    let sparkline_width = args.flag_width.get();
    let sparkline_height = args.flag_height.get();
    let max_bins = cols_for_sparkline / sparkline_width;

    let mut sparkline_renderer_options = SparklineRendererOptions::new();
    sparkline_renderer_options.width = sparkline_width;
    sparkline_renderer_options.height = sparkline_height;

    let mut sparkline_renderer = sparkline_renderer_options.build();

    // Rendering
    for mut series in pool.into_iter() {
        if series.len() > max_bins {
            series.discretize(max_bins);
        }

        let scale = series.to_scale(ScaleType::Linear).unwrap();
        sparkline_renderer.render(&scale, &series.numbers);

        writeln!(&mut out, "{}", sparkline_renderer)?;
    }

    Ok(())
}
