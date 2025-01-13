use std::num::NonZeroUsize;

use colored::Colorize;
use colorgrad::Gradient;
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

// TODO: degraded ratio version where the number is printed

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
}

static USAGE: &str = "
TODO...

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --help

heatmap options:
    -S, --scale <n>          Size of the heatmap square in terminal rows.
                             [default: 1]
    -C, --force-colors       Force colors even if output is not supposed to be able to
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
    flag_scale: NonZeroUsize,
    flag_force_colors: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    if args.flag_force_colors {
        colored::control::set_override(true);
    }

    let gradient = colorgrad::preset::or_rd();

    let mut rdr = conf.reader()?;
    let mut record = csv::ByteRecord::new();

    let column_labels = rdr
        .headers()?
        .iter()
        .skip(1)
        .map(String::from)
        .collect::<Vec<_>>();

    let mut matrix = Matrix::from_column_labels(column_labels);

    while rdr.read_byte_record(&mut record)? {
        let label = String::from_utf8(record[0].to_vec()).expect("could not decode utf8");

        let row = record
            .iter()
            .skip(1)
            .map(|cell| fast_float::parse::<f64, &[u8]>(cell).ok())
            .collect::<Vec<_>>();

        matrix.push_row(label, row);
    }

    let cols = util::acquire_term_cols(&None);
    let label_cols =
        ((cols as f64 * 0.3).floor() as usize).min(matrix.max_row_label_width().unwrap() + 1);
    let left_padding = " ".repeat(label_cols);

    let (min, max) = matrix.extent.unwrap();
    let domain_width = max - min;
    let scale = args.flag_scale.get();

    let column_info = matrix
        .column_labels
        .iter()
        .enumerate()
        .map(|(i, label)| format!("{}: {}", (i + 1).to_string().dimmed(), label))
        .collect::<Vec<_>>()
        .join(" ");

    println!(
        "{}{}",
        left_padding,
        util::unicode_aware_wrap(&column_info, cols.saturating_sub(label_cols), label_cols)
    );
    println!();

    print!("{}", left_padding);
    for i in 0..matrix.column_labels.len() {
        print!(
            "{}",
            util::unicode_aware_rpad(&(i + 1).to_string(), 2 * scale, " "),
        );
    }
    println!();

    for (row_label, row) in matrix.rows() {
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

            for cell in row {
                match cell {
                    None => print!("{}", "  ".repeat(scale)),
                    Some(f) => {
                        let normalized = (f - min) / domain_width;

                        let color = gradient.at(normalized as f32).to_rgba8();
                        print!(
                            "{}",
                            "  ".repeat(scale)
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
