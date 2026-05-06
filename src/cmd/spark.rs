use std::io::{stdout, Write};
use std::num::NonZeroUsize;

use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::scales::{ExtentBuilder, Scale, ScaleType};
use crate::util::{self, ColorMode};
use crate::CliResult;

// TODO: use space to symbolize zero when we can
// TODO: diverging scales
// TODO: stripes and colors
// TODO: flag to allow full height
// TODO: dist highlight mean, median
// TODO: dist, freqdist, loc scale, unpivote, temporal
// TODO: -w, -h
// TODO: share y scale across series
// TODO: joy div plot
// TODO: streaming version when possible (pivoted)
// TODO: unpivot flag, working with groupby
// TODO: --rainbow, horizontal and vertical (also for stripes)

// NOTE: last char is only used when stacking through -H/--height
static SPARKLINE_CHARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

#[derive(Debug)]
struct SeriesBuilder {
    extent: ExtentBuilder<f64>,
    numbers: Vec<f64>,
}

impl SeriesBuilder {
    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        Self {
            extent: ExtentBuilder::new(),
            numbers: Vec::with_capacity(capacity),
        }
    }

    #[inline]
    fn push(&mut self, x: f64) {
        self.numbers.push(x);
        self.extent.process(x);
    }

    #[inline]
    fn try_push(&mut self, cell: &[u8]) -> CliResult<()> {
        let x = fast_float::parse(cell)?;
        self.push(x);
        Ok(())
    }

    fn discretize(&mut self, count: usize) {
        if count < self.numbers.len() {
            self.extent.clear();

            let mut bins = vec![0.0; count];
            let chunk_size = (self.numbers.len() as f64 / count as f64).ceil() as usize;

            for (i, chunk) in self.numbers.chunks(chunk_size).enumerate() {
                for x in chunk.iter().copied() {
                    bins[i] += x;
                }

                self.extent.process(bins[i]);
            }

            self.numbers = bins;
        } else {
            unimplemented!()
        }
    }

    fn to_scale(&self, scale_type: ScaleType) -> Option<Scale> {
        self.extent
            .build()
            .map(|extent| Scale::new(scale_type, (extent.min(), extent.max()), (0.0, 1.0)))
    }
}

static USAGE: &str = "
TODO...

Usage:
    xan spark [options] [<input>]
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
                      Can also be given as a ratio of the terminal's width e.g. \"0.5\".
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
    arg_input: Option<String>,
    flag_width: NonZeroUsize,
    flag_height: NonZeroUsize,
    flag_cols: Option<String>,
    flag_color: ColorMode,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if matches!(args.arg_input.as_deref(), Some("debate")) {
        println!(
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
        .no_headers(args.flag_no_headers);

    let mut reader = rconf.simd_reader()?;
    let headers = reader.byte_headers()?.clone();

    let mut record = ByteRecord::new();

    let mut pool: Vec<SeriesBuilder> = Vec::new();

    // Aggregating data
    while reader.read_byte_record(&mut record)? {
        let mut series_builder = SeriesBuilder::with_capacity(headers.len());

        for cell in record.iter() {
            series_builder.try_push(cell)?;
        }

        pool.push(series_builder);
    }

    let cols_for_sparkline = cols;
    let sparkline_width = args.flag_width.get();
    let sparkline_height = args.flag_height.get();

    // Rendering
    for mut series_builder in pool.into_iter() {
        series_builder.discretize(cols_for_sparkline / sparkline_width);
        let scale = series_builder.to_scale(ScaleType::Linear).unwrap();

        for _h in 0..sparkline_height {
            let max_index = SPARKLINE_CHARS.len() - 1;

            for x in series_builder.numbers.iter().copied() {
                let sparkline_char = if x == 0.0 {
                    ' '
                } else {
                    let mut bar_index = (scale.percent(x) * max_index as f64).floor() as usize;
                    bar_index = bar_index.min(max_index - 1);
                    SPARKLINE_CHARS[bar_index]
                };

                for _ in 0..sparkline_width {
                    write!(&mut out, "{}", sparkline_char)?;
                }
            }

            writeln!(&mut out)?
        }
    }

    Ok(())
}
