use std::num::NonZeroUsize;

use colored::{ColoredString, Colorize};
use numfmt::{Formatter, Precision};
use serde::de::{Deserialize, Deserializer, Error};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::scales::{Extent, ExtentBuilder, GradientName, LinearScale};
use crate::util;
use crate::CliResult;

// Taken from: https://stackoverflow.com/questions/3942878/how-to-decide-font-color-in-white-or-black-depending-on-background-color
fn text_should_be_black(color: &[u8; 4]) -> bool {
    (color[0] as f32 * 0.299 + color[1] as f32 * 0.587 + color[2] as f32 * 0.114) > 150.0
}

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

impl<'de> Deserialize<'de> for Normalization {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;

        Ok(match raw.as_str() {
            "all" | "full" => Self::Full,
            "col" | "cols" | "column" | "columns" => Self::Column,
            "row" | "rows" => Self::Row,
            _ => {
                return Err(D::Error::custom(format!(
                    "unsupported normalization \"{}\"",
                    &raw
                )))
            }
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
Draw a heatmap from CSV data.

Use the --show-gradients flag to display a showcase of available
color gradients.

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --show-gradients
    xan heatmap --green-hills
    xan heatmap --help

heatmap options:
    -G, --gradient <name>  Gradient to use. Use --show-gradients to see what is
                           available.
                           [default: or_rd]
    -m, --min <n>          Minimum value for a cell in the heatmap. Will clamp
                           irrelevant values and use this min for normalization.
    -M, --max <n>          Maximum value for a cell in the heatmap. Will clamp
                           irrelevant values and use this max for normalization.
    --normalize <mode>     How to normalize the heatmap's values. Can be one of
                           \"full\", \"row\" or \"col\".
                           [default: full]
    -S, --size <n>         Size of the heatmap square in terminal rows.
                           [default: 1]
    -D, --diverging        Use a diverging color gradient. Currently only shorthand
                           for \"--gradient rd_bu\".
    --cram                 Attempt to cram column labels over the columns.
                           Usually works better when -S, --scale > 1.
    -N, --show-numbers     Whether to attempt to show numbers in the cells.
                           Usually only useful when -S, --scale > 1.
    -C, --force-colors     Force colors even if output is not supposed to be able to
                           handle them.
    --show-gradients       Display a showcase of available gradients.

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
    flag_gradient: GradientName,
    flag_min: Option<f64>,
    flag_max: Option<f64>,
    flag_size: NonZeroUsize,
    flag_normalize: Normalization,
    flag_diverging: bool,
    flag_cram: bool,
    flag_show_numbers: bool,
    flag_force_colors: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_show_gradients: bool,
    flag_green_hills: bool,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_diverging && self.flag_gradient.as_str() == "or_rd" {
            self.flag_gradient = GradientName::RdBu;
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    if args.flag_show_gradients {
        println!("{}", "Sequential scales".bold());
        println!();
        for gradient_name in GradientName::sequential_iter() {
            println!("{}", gradient_name.as_str());
            println!("{}", gradient_name.sample());
        }
        println!("\n");

        println!("{}", "Diverging scales".bold());
        println!();
        for gradient_name in GradientName::diverging_iter() {
            println!("{}", gradient_name.as_str());
            println!("{}", gradient_name.sample());
        }
        println!();

        return Ok(());
    }

    if args.flag_green_hills {
        print_green_hills();
        return Ok(());
    }

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    if args.flag_force_colors {
        colored::control::set_override(true);
    }

    let forced_extent = (args.flag_min, args.flag_max);

    let gradient = args.flag_gradient.build();

    let mut rdr = conf.reader()?;
    let mut record = csv::ByteRecord::new();

    let column_labels = rdr
        .headers()?
        .iter()
        .skip(1)
        .map(String::from)
        .collect::<Vec<_>>();

    let mut formatter = args
        .flag_show_numbers
        .then(|| Formatter::new().precision(Precision::Significance(args.flag_size.get() as u8)));

    let mut matrix = Matrix::new(column_labels, forced_extent);

    while rdr.read_byte_record(&mut record)? {
        let label = util::sanitize_text_for_single_line_printing(
            std::str::from_utf8(&record[0]).expect("could not decode utf8"),
        );

        let row = record
            .iter()
            .skip(1)
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

    // Printing column info
    let column_info = matrix
        .column_labels
        .iter()
        .enumerate()
        .map(|(i, label)| format!("{}: {}", (i + 1).to_string().dimmed(), label))
        .collect::<Vec<_>>()
        .join(" ");

    if !args.flag_cram {
        println!(
            "{}{}",
            left_padding,
            util::unicode_aware_wrap(&column_info, cols.saturating_sub(label_cols), label_cols)
        );
        println!();
    }

    print!("{}", left_padding);
    for (i, col_label) in matrix.column_labels.iter().enumerate() {
        let label = if !args.flag_cram {
            (i + 1).to_string()
        } else {
            col_label.to_string()
        };

        print!(
            "{}",
            util::unicode_aware_rpad_with_ellipsis(&label, 2 * size, " "),
        );
    }
    println!();

    // Printing rows
    let midpoint = size / 2;

    let col_scales = args.flag_normalize.is_column().then(|| {
        matrix
            .extent_per_column(forced_extent)
            .into_iter()
            .map(|extent_opt| extent_opt.map(LinearScale::from_extent))
            .collect::<Vec<_>>()
    });

    for (row_label, row) in matrix.rows() {
        let row_scale = args
            .flag_normalize
            .is_row()
            .then(|| compute_row_extent(row, forced_extent).map(LinearScale::from_extent));

        for i in 0..size {
            if i == 0 {
                print!(
                    "{} ",
                    util::unicode_aware_rpad_with_ellipsis(
                        row_label,
                        label_cols.saturating_sub(1),
                        " "
                    )
                );
            } else {
                print!("{}", left_padding);
            }

            for (col_i, cell) in row.iter().enumerate() {
                match cell {
                    None => print!("{}", "  ".repeat(size)),
                    Some(f) => {
                        let scale_opt = row_scale.clone().unwrap_or_else(|| {
                            col_scales
                                .as_ref()
                                .and_then(|scales| scales[col_i].clone())
                                .or_else(|| full_scale.clone())
                        });

                        let color_opt =
                            scale_opt.map(|scale| scale.map_color(&gradient, *f).to_rgba8());

                        let body = match formatter.as_mut() {
                            Some(fmt) if i == midpoint => {
                                let formatted = util::unicode_aware_ellipsis(
                                    &util::format_number_with_formatter(fmt, *f),
                                    size * 2,
                                );

                                format!(
                                    "{:^width$}",
                                    match color_opt {
                                        Some(color) =>
                                            if text_should_be_black(&color) {
                                                formatted.black()
                                            } else {
                                                formatted.normal()
                                            },
                                        None => formatted.normal(),
                                    },
                                    width = size * 2
                                )
                            }
                            _ => " ".repeat(size * 2),
                        };

                        print!(
                            "{}",
                            if let Some(color) = color_opt {
                                body.on_truecolor(color[0], color[1], color[2])
                            } else {
                                body.normal()
                            }
                        );
                    }
                }
            }
            println!();
        }
    }

    println!();

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

fn print_green_hills() {
    use bstr::ByteSlice;

    for row in GREEN_HILLS
        .trim()
        .iter()
        .filter(|c| **c != 10)
        .cloned()
        .collect::<Vec<_>>()
        .chunks(GREEN_HILLS_COLS as usize)
    {
        for code in row.trim() {
            print!("{}", resolve_green_hill_code(*code, " "));
        }
        println!();
    }

    println!();
}
