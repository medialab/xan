use std::collections::HashMap;
use std::io;

use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType};
use ratatui::Terminal;

use config::{Config, Delimiter};
use util::{self, ImmutableRecordHelpers};
use CliResult;

static USAGE: &str = "
TODO...

Usage:
    xsv scatter [options] <x-column> <y-column> [<input>]
    xsv scatter --help

scatter options:
    --color <column>  Name of the categorical column that will be used to
                      color the different points.
    --cols <num>      Width of the graph in terminal columns, i.e. characters.
                      Defaults to using all your terminal's width or 80 if
                      terminal's size cannot be found (i.e. when piping to file).

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_x_column: String,
    arg_y_column: String,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_cols: Option<usize>,
    flag_color: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    // Collecting data
    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?;

    let x_column_index = headers
        .find_column_index(args.arg_x_column.as_bytes())
        .ok_or_else(|| {
            format!(
                "cannot find column containing x values \"{}\"",
                args.arg_x_column
            )
        })?;

    let y_column_index = headers
        .find_column_index(args.arg_y_column.as_bytes())
        .ok_or_else(|| {
            format!(
                "cannot find column containing y values \"{}\"",
                args.arg_y_column
            )
        })?;

    let color_column_index = args
        .flag_color
        .as_ref()
        .map(|name| {
            headers
                .find_column_index(name.as_bytes())
                .ok_or_else(|| format!("cannot find column containing color values \"{}\"", name))
        })
        .transpose()?;

    let mut record = csv::ByteRecord::new();

    let mut x_series = Series::new();
    let mut y_series = Series::new();

    let mut grouped_series: HashMap<Vec<u8>, (Series, Series)> = HashMap::new();

    while rdr.read_byte_record(&mut record)? {
        let x = String::from_utf8_lossy(&record[x_column_index])
            .parse()
            .expect("could not parse number");
        let y = String::from_utf8_lossy(&record[y_column_index])
            .parse()
            .expect("could not parse number");

        if let Some(i) = color_column_index {
            let color_value = record[i].to_vec();

            grouped_series
                .entry(color_value)
                .and_modify(|(xs, ys)| {
                    xs.add(x);
                    ys.add(x);
                })
                .or_insert_with(|| (Series::of(x), Series::of(y)));
        } else {
            x_series.add(x);
            y_series.add(y);
        }
    }

    // Drawing
    let rows = util::acquire_term_rows().unwrap_or(20) as u16;
    let cols = util::acquire_term_cols(&args.flag_cols) as u16;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;

    terminal.draw(|frame| {
        let area = Rect::new(0, 0, cols, rows.saturating_sub(1));
        // Create the datasets to fill the chart with

        let points = x_series
            .values
            .iter()
            .copied()
            .zip(y_series.values.iter().copied())
            .collect::<Vec<_>>();

        let datasets = vec![Dataset::default()
            .name("csv")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Scatter)
            .style(Style::default().cyan())
            .data(&points)];

        // Create the X axis and define its properties
        let x_axis = Axis::default()
            .title(args.arg_x_column.red())
            .style(Style::default().white())
            .bounds(x_series.domain().unwrap())
            .labels(x_series.graduations().unwrap());

        // Create the Y axis and define its properties
        let y_axis = Axis::default()
            .title(args.arg_y_column.red())
            .style(Style::default().white())
            .bounds(y_series.domain().unwrap())
            .labels(y_series.graduations().unwrap());

        // Create the chart and link all the parts together
        let chart = Chart::new(datasets)
            .block(Block::default())
            .x_axis(x_axis)
            .y_axis(y_axis);

        frame.render_widget(chart, area);
    })?;

    Ok(())
}

struct Series {
    values: Vec<f64>,
    extent: Option<(f64, f64)>,
}

impl Series {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            extent: None,
        }
    }

    fn of(value: f64) -> Self {
        Self {
            values: vec![value],
            extent: Some((value, value)),
        }
    }

    fn add(&mut self, value: f64) {
        self.values.push(value);

        self.extent = match self.extent {
            None => Some((value, value)),
            Some((mut min, mut max)) => {
                if value < min {
                    min = value;
                }
                if value > max {
                    max = value;
                }

                Some((min, max))
            }
        }
    }

    fn domain(&self) -> Option<[f64; 2]> {
        self.extent.map(|(min, max)| [min, max])
    }

    fn graduations(&self) -> Option<Vec<Span>> {
        self.extent
            .map(|(min, max)| vec![Span::from(min.to_string()), Span::from(max.to_string())])
    }
}
