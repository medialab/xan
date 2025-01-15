use std::num::NonZeroUsize;

use colored::{ColoredString, Colorize};
use colorgrad::Gradient;
use numfmt::{Formatter, Precision};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

// TODO: finish normalize modes
// TODO: failure when input file is emptyish

// Taken from: https://stackoverflow.com/questions/3942878/how-to-decide-font-color-in-white-or-black-depending-on-background-color
fn text_should_be_black(color: &[u8; 4]) -> bool {
    (color[0] as f32 * 0.299 + color[1] as f32 * 0.587 + color[2] as f32 * 0.114) > 150.0
}

#[derive(Debug)]
struct Matrix {
    array: Vec<Option<f64>>,
    column_labels: Vec<String>,
    row_labels: Vec<String>,
    extent: Option<(f64, f64)>,
}

impl Matrix {
    fn from_column_labels(column_labels: Vec<String>) -> Self {
        Self {
            array: Vec::new(),
            column_labels,
            row_labels: Vec::new(),
            extent: None,
        }
    }

    fn push_row<I>(&mut self, label: String, row: I)
    where
        I: IntoIterator<Item = Option<f64>>,
    {
        self.row_labels.push(label);

        for cell in row {
            self.array.push(cell);

            if let Some(f) = cell {
                match self.extent.as_mut() {
                    None => self.extent = Some((f, f)),
                    Some((min, max)) => {
                        if f < *min {
                            *min = f;
                        }

                        if f > *max {
                            *max = f;
                        }
                    }
                }
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

    fn extent_per_column(&self) -> Vec<Option<(f64, f64)>> {
        let mut cols = vec![None; self.column_labels.len()];

        for rows in self.rows() {
            for (i, cell) in rows.1.iter().enumerate() {
                let current = &mut cols[i];

                if let Some(f) = cell {
                    match current {
                        None => *current = Some((*f, *f)),
                        Some((min, max)) => {
                            if f < min {
                                *min = *f;
                            }
                            if f > max {
                                *max = *f;
                            }
                        }
                    }
                }
            }
        }

        cols
    }
}

static USAGE: &str = "
Draw a heatmap from CSV data.

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --green-hills
    xan heatmap --help

heatmap options:
    -m, --min <n>       Minimum value for a cell in the heatmap. Will clamp
                        irrelevant values and use this min for normalization.
    -M, --max <n>       Maximum value for a cell in the heatmap. Will clamp
                        irrelevant values and use this max for normalization.
    --normalize <mode>  How to normalize the heatmap's values. Can be one of
                        \"full\", \"row\" or \"col\".
                        [default: full]
    -S, --scale <n>     Size of the heatmap square in terminal rows.
                        [default: 1]
    -D, --diverging     Use a diverging color gradient.
    --cram              Attempt to cram column labels over the columns.
                        Usually works better when -S, --scale > 1.
    -N, --show-numbers  Whether to attempt to show numbers in the cells.
                        Usually only useful when -S, --scale > 1.
    -C, --force-colors  Force colors even if output is not supposed to be able to
                        handle them.

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
    flag_min: Option<f64>,
    flag_max: Option<f64>,
    flag_scale: NonZeroUsize,
    flag_normalize: String,
    flag_diverging: bool,
    flag_cram: bool,
    flag_show_numbers: bool,
    flag_force_colors: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_green_hills: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

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

    let gradient = if args.flag_diverging {
        colorgrad::preset::rd_bu()
    } else {
        colorgrad::preset::or_rd()
    };

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
        .then(|| Formatter::new().precision(Precision::Significance(args.flag_scale.get() as u8)));

    let mut matrix = Matrix::from_column_labels(column_labels);

    while rdr.read_byte_record(&mut record)? {
        let label = String::from_utf8(record[0].to_vec()).expect("could not decode utf8");

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

    if let Some(extent) = matrix.extent.as_mut() {
        if let Some(min) = args.flag_min {
            extent.0 = min;
        }
        if let Some(max) = args.flag_max {
            extent.1 = max;
        }
    }

    let cols = util::acquire_term_cols(&None);
    let label_cols =
        ((cols as f64 * 0.3).floor() as usize).min(matrix.max_row_label_width().unwrap() + 1);
    let left_padding = " ".repeat(label_cols);

    let full_extent = matrix.extent.unwrap();
    let scale = args.flag_scale.get();

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
            &(i + 1).to_string()
        } else {
            col_label
        };

        print!(
            "{}",
            util::unicode_aware_rpad_with_ellipsis(label, 2 * scale, " "),
        );
    }
    println!();

    // Printing rows
    let midpoint = scale / 2;

    let col_extents = (args.flag_normalize == "col").then(|| matrix.extent_per_column());

    for (row_label, row) in matrix.rows() {
        // TODO: computations are not required each scale rows
        for i in 0..scale {
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
                    None => print!("{}", "  ".repeat(scale)),
                    Some(f) => {
                        // TODO: if the column has no value, things will get messy...
                        let (min, max) = col_extents
                            .as_ref()
                            .and_then(|extents| extents[col_i])
                            .unwrap_or(full_extent);

                        let normalized = (f - min) / (max - min);

                        let color = gradient.at(normalized as f32).to_rgba8();

                        print!(
                            "{}",
                            (match formatter.as_mut() {
                                Some(fmt) if i == midpoint => {
                                    let formatted = util::unicode_aware_ellipsis(
                                        &util::format_number_with_formatter(fmt, *f),
                                        scale * 2,
                                    );

                                    format!(
                                        "{:^width$}",
                                        if text_should_be_black(&color) {
                                            formatted.black()
                                        } else {
                                            formatted.normal()
                                        },
                                        width = scale * 2
                                    )
                                }
                                _ => " ".repeat(scale * 2),
                            })
                            .on_truecolor(color[0], color[1], color[2])
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
