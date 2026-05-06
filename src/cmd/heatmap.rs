use std::convert::TryFrom;
use std::io::{stdout, Write};
use std::iter::repeat_n;
use std::num::NonZeroUsize;

use colored::{ColoredString, Colorize};
use numfmt::{Formatter, Precision};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::scales::{Extent, ExtentBuilder, GradientName, LinearScale};
use crate::select::{SelectedColumns, Selection};
use crate::util::{self, ColorMode};
use crate::CliResult;

static ASCII_GRADIENT: [char; 4] = ['░', '▒', '▓', '█'];

// Taken from: https://stackoverflow.com/questions/3942878/how-to-decide-font-color-in-white-or-black-depending-on-background-color
fn text_should_be_black(color: &[u8; 4]) -> bool {
    (color[0] as f32 * 0.299 + color[1] as f32 * 0.587 + color[2] as f32 * 0.114) > 150.0
}

#[derive(Deserialize)]
enum CramMode {
    Auto,
    Always,
    Never,
}

#[derive(Deserialize)]
enum Alignment {
    Left,
    Center,
    Right,
}

#[derive(Deserialize)]
#[serde(try_from = "String")]
enum Normalization {
    Full,
    Column,
    Row,
}

impl Normalization {
    fn is_column(&self) -> bool {
        matches!(self, Self::Column)
    }

    fn is_row(&self) -> bool {
        matches!(self, Self::Row)
    }
}

impl TryFrom<String> for Normalization {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_str() {
            "all" | "full" => Self::Full,
            "col" | "cols" | "column" | "columns" => Self::Column,
            "row" | "rows" => Self::Row,
            _ => return Err(format!("unsupported normalization \"{}\"", &value)),
        })
    }
}

#[derive(Debug)]
struct Matrix {
    array: Vec<Option<f64>>,
    column_labels: Vec<String>,
    row_labels: Vec<String>,
    extent_builder: ExtentBuilder<f64>,
    extent: Option<Extent<f64>>,
}

impl Matrix {
    fn new(column_labels: Vec<String>, forced_extent: (Option<f64>, Option<f64>)) -> Self {
        Self {
            array: Vec::new(),
            column_labels,
            row_labels: Vec::new(),
            extent_builder: ExtentBuilder::from(forced_extent),
            extent: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.array.is_empty()
    }

    fn finalize(&mut self) {
        self.extent = self.extent_builder.clone().build();
    }

    fn push_row<I>(&mut self, label: String, row: I)
    where
        I: IntoIterator<Item = Option<f64>>,
    {
        self.row_labels.push(label);

        for cell in row {
            self.array.push(cell);

            if let Some(f) = cell {
                self.extent_builder.process(f);
            }
        }
    }

    fn rows(&self) -> impl Iterator<Item = (&String, &[Option<f64>])> {
        self.array
            .chunks(self.column_labels.len())
            .enumerate()
            .map(|(i, chunk)| (&self.row_labels[i], chunk))
    }

    fn max_row_label_width(&self) -> Option<usize> {
        self.row_labels.iter().map(|label| label.width()).max()
    }

    fn extent_per_column(
        &self,
        forced_extent: (Option<f64>, Option<f64>),
    ) -> Vec<Option<Extent<f64>>> {
        let mut cols: Vec<_> = (0..self.column_labels.len())
            .map(|_| ExtentBuilder::from(forced_extent))
            .collect();

        for rows in self.rows() {
            for (i, cell) in rows.1.iter().enumerate() {
                let current = &mut cols[i];

                if let Some(f) = cell {
                    current.process(*f);
                }
            }
        }

        cols.into_iter().map(|builder| builder.build()).collect()
    }
}

fn compute_row_extent(
    row: &[Option<f64>],
    forced_extent: (Option<f64>, Option<f64>),
) -> Option<Extent<f64>> {
    let mut extent_builder = ExtentBuilder::from(forced_extent);

    for cell in row.iter().copied().flatten() {
        extent_builder.process(cell);
    }

    extent_builder.build()
}

static USAGE: &str = "
Render CSV data as a heatmap grid. x-axis labels will be taken from file's headers
(or 0-based column indices when used with -n/--no-headers). While y-axis labels
will be taken from the file's first column by default. All columns beyond the
first one will be considered as numerical and used to draw the heatmap grid.

If your file is not organized thusly, you can still use the -l/--label flag
to select the y-axis label column and/or the -v/--values flag to select columns
to be considered to draw the heatmap grid.

This command is typically used to display the results of `xan matrix`. For instance,
here is how to draw a correlation matrix:

    $ xan matrix corr -s 'sepal_*,petal_*' iris.csv | xan heatmap --diverging --unit

Here is another example drawing an adjacency matrix:

    $ xan matrix adj source target edges.csv | xan heatmap

Note that drawn matrices do not have to be square and can really be anything.
It is possible to think of the result as the symbolic representation of given
tabular data where each cell is represented by a square with a continuous color.

Consider the following example, for instance, where we draw a heatmap of Twitter
account popularity profiles wrt retweets, replies and likes:

    $ xan groupby user_screen_name \\
    $   'mean(retweet_count) as rt, mean(reply_count) as rp, mean(like_count) as lk' \\
    $   tweets.csv | \\
    $ xan heatmap --size 2 --cram --show-numbers

You can also achieve a result similar to conditional formatting in a spreadsheet
by leveraging the -W/--width flag and showing numbers thusly:

    $ xan matrix count lang1 lang2 data.csv | xan heatmap -W 6 --show-numbers

Note that, by default, since there is not enough place on the x-axis, labels will be
printed in a legend before the heatmap itself. If you can afford the space, feel
free to use a -S/--size greater then 1 and toggle the -C/--cram flag to fit the
labels on top of the x-axis instead.

Increasing -S/--size also means you can try fitting the numbers within the heatmap's
cells themselves using -N/--show-numbers.

Finally, if you want a showcase of available color gradients, use the --show-gradients
flag.

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --show-gradients
    xan heatmap --green-hills
    xan heatmap --help

heatmap options:
    -l, --label <column>    Column containing the y-axis labels. Defaults to
                            the first column of the file.
    -v, --values <columns>  Columns containing numerical values to display in the
                            heatmap. Defaults to all columns of the file beyond
                            the first one.
    -G, --gradient <name>   Gradient to use. Use --show-gradients to see what is
                            available.
                            [default: or_rd]
    -A, --ascii             Use ascii shade characters (░▒▓█) to draw the heatmap instead
                            of coloring cell backgrounds. The output can therefore
                            be copy-pasted, but is restricted to a 4 steps gradient.
                            Does not work with -N/--show-numbers nor -Z/--show-normalized.
    -m, --min <n>           Minimum value for a cell in the heatmap. Will clamp
                            irrelevant values and use this min for normalization.
    -M, --max <n>           Maximum value for a cell in the heatmap. Will clamp
                            irrelevant values and use this max for normalization.
    -U, --unit              Shorthand for --min 0, --max 1 or --min -1, --max 1 when
                            using -D/--diverging.
    --normalize <mode>      How to normalize the heatmap's values. Can be one of
                            \"full\", \"row\" or \"col\".
                            [default: full]
    -S, --size <n>          Size of the heatmap square in terminal rows.
                            [default: 1]
    -W, --width <n>         Use this to set heatmap grid cells width if you want
                            rectangles instead of squares and want to have more
                            space to display cell numbers with -N/--show-numbers
                            or -Z/--show-normalized.
    -D, --diverging         Use a diverging color gradient. Currently only shorthand
                            for \"--gradient rd_bu\".
    -C, --cram <choice>     Whether to cram x-axis labels over the heatmap grid columns.
                            Can be either \"auto\", \"always\" or \"never\".
                            [default: auto]
    -N, --show-numbers      Whether to attempt to show numbers in the cells.
                            Usually only useful when -S/--size > 1.
                            Cannot be used with -Z/--show-normalized.
    -Z, --show-normalized   Whether to attempt to show normalized numbers in the
                            cells. Usually only useful when -S/--size > 1.
                            Cannot be used with -N/--show-numbers.
    -a, --align <choice>    How to align numbers in the cell when shown. Can be
                            either \"left\", \"center\" or \"right\".
                            [default: center]
    -F, --fill              Whether to fill empty cells with the \"⡪\" character.
    --repeat-headers <n>    Repeat headers every <n> heatmap rows. This can also
                            be set to \"auto\" to choose a suitable number based
                            on the height of your terminal.
    --show-gradients        Display a showcase of available gradients.
    --color <when>          When to color the output using ANSI escape codes.
                            Use `auto` for automatic detection, `never` to
                            disable colors completely and `always` to force
                            colors, even when the output could not handle them.
                            [default: auto]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_label: Option<SelectedColumns>,
    flag_values: Option<SelectedColumns>,
    flag_gradient: GradientName,
    flag_ascii: bool,
    flag_min: Option<f64>,
    flag_max: Option<f64>,
    flag_unit: bool,
    flag_size: NonZeroUsize,
    flag_width: Option<NonZeroUsize>,
    flag_normalize: Normalization,
    flag_diverging: bool,
    flag_cram: CramMode,
    flag_show_numbers: bool,
    flag_show_normalized: bool,
    flag_align: Alignment,
    flag_fill: bool,
    flag_color: ColorMode,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_repeat_headers: Option<String>,
    flag_show_gradients: bool,
    flag_green_hills: bool,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_unit {
            self.flag_min = self
                .flag_min
                .or(Some(if self.flag_diverging { -1.0 } else { 0.0 }));
            self.flag_max = self.flag_max.or(Some(1.0));
        }

        if self.flag_diverging && self.flag_gradient.as_str() == "or_rd" {
            self.flag_gradient = GradientName::RdBu;
        }
    }

    fn resolve_repeat_headers(&self) -> CliResult<Option<usize>> {
        match &self.flag_repeat_headers {
            None => Ok(None),
            Some(n) => {
                if n == "auto" {
                    Ok(Some(
                        util::acquire_term_rows(&None).saturating_sub(2).max(1)
                            / self.flag_size.get(),
                    ))
                } else {
                    match n.parse::<NonZeroUsize>() {
                        Ok(i) => Ok(Some(i.get())),
                        Err(_) => Err(From::from(format!(
                            "expected --repeat-headers to be \"auto\" or a positive integer but got {}!",
                            n
                        ))),
                    }
                }
            }
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();
    args.flag_color.apply();

    let repeat_headers_opt = args.resolve_repeat_headers()?;

    if args.flag_ascii && (args.flag_show_numbers || args.flag_show_normalized) {
        Err("-A/--ascii does not work with -N/--show-numbers nor -Z/--show-normalized!")?;
    }

    if args.flag_show_numbers && args.flag_show_normalized {
        Err("only one of -N/--show-numbers or -Z/--show-normalized must be given!")?;
    }

    let mut out = stdout();

    if args.flag_show_gradients {
        writeln!(&mut out, "{}", "Sequential scales (Single-Hue)".bold())?;
        writeln!(&mut out)?;
        for gradient_name in GradientName::single_hue_sequential_iter() {
            writeln!(&mut out, "{}", gradient_name.as_str())?;
            writeln!(&mut out, "{}", gradient_name.sample())?;
        }
        writeln!(&mut out, "\n")?;

        writeln!(&mut out, "{}", "Sequential scales (Multi-Hue)".bold())?;
        writeln!(&mut out)?;
        for gradient_name in GradientName::multi_hue_sequential_iter() {
            writeln!(&mut out, "{}", gradient_name.as_str())?;
            writeln!(&mut out, "{}", gradient_name.sample())?;
        }
        writeln!(&mut out, "\n")?;

        writeln!(&mut out, "{}", "Diverging scales".bold())?;
        writeln!(&mut out)?;
        for gradient_name in GradientName::diverging_iter() {
            writeln!(&mut out, "{}", gradient_name.as_str())?;
            writeln!(&mut out, "{}", gradient_name.sample())?;
        }
        writeln!(&mut out, "\n")?;

        writeln!(&mut out, "{}", "Cyclical scales".bold())?;
        writeln!(&mut out)?;
        for gradient_name in GradientName::cyclical_iter() {
            writeln!(&mut out, "{}", gradient_name.as_str())?;
            writeln!(&mut out, "{}", gradient_name.sample())?;
        }
        writeln!(&mut out)?;

        return Ok(());
    }

    if args.flag_green_hills {
        print_green_hills()?;
        return Ok(());
    }

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let forced_extent = (args.flag_min, args.flag_max);

    let gradient = args.flag_gradient.build();

    let mut rdr = conf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let label_column_index = match &args.flag_label {
        Some(flag_label) => flag_label.single_selection(&headers, !conf.no_headers)?,
        None => 0,
    };

    let mut values_sel = match &args.flag_values {
        Some(flag_values) => flag_values.selection(&headers, !conf.no_headers)?,
        None => Selection::without_indices(headers.len(), &[label_column_index]),
    };

    values_sel.dedup();

    if values_sel.contains(label_column_index) {
        Err("-l/--label column must not be part of columns selected by -v/--values!")?;
    }

    let mut record = simd_csv::ByteRecord::new();

    let mut column_labels = values_sel
        .select(&headers)
        .map(|cell| String::from_utf8_lossy(cell).into_owned())
        .collect::<Vec<_>>();

    if conf.no_headers {
        column_labels = (0..column_labels.len()).map(|i| i.to_string()).collect();
    }

    let mut matrix = Matrix::new(column_labels, forced_extent);

    while rdr.read_byte_record(&mut record)? {
        let label = util::sanitize_text_for_single_line_printing(
            std::str::from_utf8(&record[label_column_index]).expect("could not decode utf8"),
        );

        let row = values_sel
            .select(&record)
            .map(|cell| match fast_float::parse::<f64, &[u8]>(cell) {
                Ok(f) => match args.flag_min {
                    Some(min) if f < min => None,
                    _ => match args.flag_max {
                        Some(max) if f > max => None,
                        _ => Some(f),
                    },
                },
                Err(_) => None,
            })
            .collect::<Vec<_>>();

        matrix.push_row(label, row);
    }

    if matrix.is_empty() {
        Err("nothing to display!")?;
    }

    matrix.finalize();

    let cols = util::acquire_term_cols(&None);
    let label_cols =
        ((cols as f64 * 0.3).floor() as usize).min(matrix.max_row_label_width().unwrap() + 1);
    let left_padding = " ".repeat(label_cols);

    let full_scale = matrix.extent.map(LinearScale::from_extent);

    let size = args.flag_size.get();
    let width = args.flag_width.map(NonZeroUsize::get).unwrap_or(size * 2);

    let mut formatter = (args.flag_show_numbers || args.flag_show_normalized).then(|| {
        Formatter::new().precision(Precision::Significance(width.saturating_sub(3).max(1) as u8))
    });

    // Printing column info
    let column_info = matrix
        .column_labels
        .iter()
        .enumerate()
        .map(|(i, label)| format!("{}: {}", (i + 1).to_string().dimmed(), label))
        .collect::<Vec<_>>()
        .join(" ");

    let actually_cram = match args.flag_cram {
        CramMode::Always => true,
        CramMode::Never => false,
        CramMode::Auto => {
            matrix
                .column_labels
                .iter()
                .map(|label| label.width())
                .max()
                .unwrap()
                <= width
        }
    };

    let print_legend = || -> CliResult<()> {
        writeln!(
            &out,
            "{}{}",
            left_padding,
            util::wrap(&column_info, cols.saturating_sub(label_cols), label_cols)
        )?;
        writeln!(&out)?;

        Ok(())
    };

    if !actually_cram {
        print_legend()?;
    }

    let write_headers = || -> CliResult<()> {
        write!(&out, "{}", left_padding)?;
        for (i, col_label) in matrix.column_labels.iter().enumerate() {
            let label = if !actually_cram {
                (i + 1).to_string()
            } else {
                col_label.to_string()
            };

            write!(
                &out,
                "{}",
                util::unicode_aware_rpad_with_ellipsis(&label, width, " "),
            )?;
        }
        writeln!(&out)?;

        Ok(())
    };

    write_headers()?;

    // Printing rows
    let midpoint = size / 2;

    let col_scales = args.flag_normalize.is_column().then(|| {
        matrix
            .extent_per_column(forced_extent)
            .into_iter()
            .map(|extent_opt| extent_opt.map(LinearScale::from_extent))
            .collect::<Vec<_>>()
    });

    for (row_i, (row_label, row)) in matrix.rows().enumerate() {
        if let Some(repeat_headers_limit) = repeat_headers_opt {
            if row_i > 0 && row_i % repeat_headers_limit == 0 {
                if !actually_cram {
                    writeln!(&out)?;
                    print_legend()?;
                }

                write_headers()?;
            }
        }

        let row_scale = args
            .flag_normalize
            .is_row()
            .then(|| compute_row_extent(row, forced_extent).map(LinearScale::from_extent));

        for i in 0..size {
            if i == 0 {
                write!(
                    &out,
                    "{} ",
                    util::unicode_aware_rpad_with_ellipsis(
                        row_label,
                        label_cols.saturating_sub(1),
                        " "
                    )
                )?;
            } else {
                write!(&out, "{}", left_padding)?;
            }

            for (col_i, cell) in row.iter().enumerate() {
                match cell {
                    None => write!(
                        &out,
                        "{}",
                        (if args.flag_fill { "⡪" } else { " " })
                            .repeat(width)
                            .dimmed()
                    )?,
                    Some(f) => {
                        let scale_opt = match &row_scale {
                            Some(s) => s.as_ref(),
                            None => match &col_scales {
                                Some(ss) => ss[col_i].as_ref(),
                                None => full_scale.as_ref(),
                            },
                        };

                        let percent_opt = scale_opt.map(|scale| {
                            let p = scale.percent(*f);

                            // NOTE: f  or now, if scale's domain is constant,
                            // we fallback to the midpoint. We might revise this
                            // in the future.
                            if p.is_nan() {
                                0.5
                            } else {
                                p
                            }
                        });

                        if args.flag_ascii {
                            let ascii_gradient_index =
                                ((percent_opt.unwrap() * 4.0).floor() as usize).min(3);

                            debug_assert!(ascii_gradient_index < 4);

                            write!(
                                &out,
                                "{}",
                                repeat_n(ASCII_GRADIENT[ascii_gradient_index], width)
                                    .collect::<String>()
                            )?;
                        } else {
                            let color_opt =
                                percent_opt.map(|percent| gradient.at(percent as f32).to_rgba8());

                            let body = match formatter.as_mut() {
                                Some(fmt) if i == midpoint => {
                                    let formatted = util::unicode_aware_ellipsis(
                                        &util::format_number_with_formatter(
                                            fmt,
                                            if args.flag_show_normalized {
                                                percent_opt.unwrap()
                                            } else {
                                                *f
                                            },
                                        ),
                                        width,
                                    );

                                    let colored_number = match color_opt {
                                        Some(color) => {
                                            if text_should_be_black(&color) {
                                                formatted.black()
                                            } else {
                                                formatted.normal()
                                            }
                                        }
                                        None => formatted.normal(),
                                    };

                                    match args.flag_align {
                                        Alignment::Left => {
                                            format!("{:<width$}", colored_number, width = width)
                                        }
                                        Alignment::Center => {
                                            format!("{:^width$}", colored_number, width = width)
                                        }
                                        Alignment::Right => {
                                            format!("{:>width$}", colored_number, width = width)
                                        }
                                    }
                                }
                                _ => " ".repeat(width),
                            };

                            write!(
                                &out,
                                "{}",
                                if let Some(color) = color_opt {
                                    body.on_truecolor(color[0], color[1], color[2])
                                } else {
                                    body.normal()
                                }
                            )?;
                        }
                    }
                }
            }
            writeln!(&out)?;
        }
    }

    writeln!(&out)?;

    Ok(())
}

static GREEN_HILLS: &[u8] = b"
gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg
rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr
eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
cccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkk
kkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccckkkkcccc
bbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmm
mmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbb
mmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbb
bbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmm
bbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmm
mmmmbbbbmmmmkkkkcccckkkkcccckkkkccccbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbb
mmmmbbbbmmmmkkkkcccckkkkcccckkkkccccbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbb
bbbbmmmmbbbbcccckkkkcccckkkkcccckkkkmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmm
bbbbmmmmbbbbcccckkkkcccckkkkcccckkkkmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmm
mmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbcccckkkkcccckkkkcccckkkk
mmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbcccckkkkcccckkkkcccckkkk
bbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmkkkkcccckkkkcccckkkkcccc
bbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmbbbbmmmmkkkkcccckkkkcccckkkkcccc
";

const GREEN_HILLS_COLS: u8 = 80;

fn resolve_green_hill_code(code: u8, string: &str) -> ColoredString {
    match code {
        // greens
        b'g' => string.on_truecolor(128, 244, 0),
        b'r' => string.on_truecolor(64, 160, 0),
        b'e' => string.on_truecolor(0, 96, 0),
        // browns
        b'b' => string.on_truecolor(96, 32, 0),
        b'm' => string.on_truecolor(191, 95, 0),
        // dark browns
        b'c' => string.on_truecolor(101, 48, 0),
        b'k' => string.on_truecolor(48, 16, 0),
        _ => unreachable!(),
    }
}

fn print_green_hills() -> CliResult<()> {
    use bstr::ByteSlice;

    let mut out = stdout();

    for row in GREEN_HILLS
        .trim()
        .iter()
        .filter(|c| **c != 10)
        .cloned()
        .collect::<Vec<_>>()
        .chunks(GREEN_HILLS_COLS as usize)
    {
        for code in row.trim() {
            write!(&mut out, "{}", resolve_green_hill_code(*code, " "))?;
        }
        writeln!(&mut out)?;
    }

    writeln!(&mut out)?;

    Ok(())
}
