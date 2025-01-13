use colored::Colorize;
use colorgrad::Gradient;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

#[derive(Debug)]
struct Matrix {
    array: Vec<f64>,
    columns: usize,
    rows: usize,
    extent: Option<(f64, f64)>,
}

impl Matrix {
    fn with_columns(columns: usize) -> Self {
        Self {
            array: Vec::new(),
            columns,
            rows: 0,
            extent: None,
        }
    }

    fn push_row<I>(&mut self, row: I)
    where
        I: IntoIterator<Item = f64>,
    {
        for cell in row {
            self.array.push(cell);

            match self.extent.as_mut() {
                None => self.extent = Some((cell, cell)),
                Some((min, max)) => {
                    if cell < *min {
                        *min = cell;
                    }

                    if cell > *max {
                        *max = cell;
                    }
                }
            }
        }

        self.rows += 1;
    }

    fn rows(&self) -> std::slice::Chunks<f64> {
        self.array.chunks(self.columns)
    }

    fn max(&self) -> Option<f64> {
        self.extent.map(|e| e.1)
    }
}

static USAGE: &str = "
TODO...

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --help

heatmap options:
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
    let headers = rdr.byte_headers()?.clone();

    let mut matrix = Matrix::with_columns(headers.len());

    while rdr.read_byte_record(&mut record)? {
        let row = record
            .iter()
            .map(|cell| fast_float::parse::<f64, &[u8]>(cell).map_err(|_| "could not parse float"))
            .collect::<Result<Vec<_>, _>>()?;

        matrix.push_row(row);
    }

    let max = matrix.max().unwrap();

    for row in matrix.rows() {
        for cell in row {
            let color = gradient.at((cell / max) as f32).to_rgba8();
            print!("{}", "██".truecolor(color[0], color[1], color[2]));
        }
        println!();
    }

    Ok(())
}
