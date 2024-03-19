use std::collections::HashMap;

use colored::{ColoredString, Colorize};

use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols;
use ratatui::text::Span;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType, LegendPosition};
use ratatui::Terminal;

use config::{Config, Delimiter};
use util::{self, ImmutableRecordHelpers};
use CliResult;

fn get_series_color(i: usize) -> Style {
    match i {
        0 => Style::default().cyan(),
        1 => Style::default().red(),
        2 => Style::default().green(),
        3 => Style::default().yellow(),
        4 => Style::default().blue(),
        5 => Style::default().magenta(),
        _ => Style::default().dim(),
    }
}

fn graduations_from_domain<'a>(min: f64, max: f64) -> Vec<Span<'a>> {
    vec![Span::from(min.to_string()), Span::from(max.to_string())]
}

static USAGE: &str = "
Draw a scatter plot or a line plot based on 2-dimensional data.

Usage:
    xsv plot [options] <x> <y> [<input>]
    xsv plot --help

plot options:
    --color <column>  Name of the categorical column that will be used to
                      color the different points.
    --cols <num>      Width of the graph in terminal columns, i.e. characters.
                      Defaults to using all your terminal's width or 80 if
                      terminal size cannot be found (i.e. when piping to file).
    --rows <num>      Height of the graph in terminal rows, i.e. characters.
                      Defaults to using all your terminal's height minus 2 or 30 if
                      terminal size cannot be found (i.e. when piping to file).

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
    arg_x: String,
    arg_y: String,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_cols: Option<usize>,
    flag_rows: Option<usize>,
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
        .find_column_index(args.arg_x.as_bytes())
        .ok_or_else(|| format!("cannot find column containing x values \"{}\"", args.arg_x))?;

    let y_column_index = headers
        .find_column_index(args.arg_y.as_bytes())
        .ok_or_else(|| format!("cannot find column containing y values \"{}\"", args.arg_y))?;

    let color_column_index = args
        .flag_color
        .as_ref()
        .map(|name| {
            headers
                .find_column_index(name.as_bytes())
                .ok_or_else(|| format!("cannot find column containing color values \"{}\"", name))
        })
        .transpose()?;

    let showing_multiple_series = color_column_index.is_some();

    let mut record = csv::ByteRecord::new();

    let mut main_series = Series::new();

    let mut grouped_series: HashMap<Vec<u8>, Series> = HashMap::new();

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
                .and_modify(|series| {
                    series.add(x, y);
                })
                .or_insert_with(|| Series::of(x, y));
        } else {
            main_series.add(x, y);
        }
    }

    // Drawing
    let rows = args
        .flag_rows
        .unwrap_or_else(|| util::acquire_term_rows().unwrap_or(30))
        .saturating_sub(2) as u16;

    let cols = util::acquire_term_cols(&args.flag_cols) as u16;

    let mut terminal = Terminal::new(TestBackend::new(cols, rows))?;

    terminal.draw(|frame| {
        let area = Rect::new(0, 0, cols, rows);

        // Create the datasets to fill the chart with
        let datasets = if showing_multiple_series {
            grouped_series
                .into_iter()
                .enumerate()
                .map(|(i, (name, _))| {
                    Dataset::default()
                        .name(String::from_utf8(name).unwrap())
                        .marker(symbols::Marker::Braille)
                        .graph_type(GraphType::Scatter)
                        .style(get_series_color(i))
                })
                .collect()
        } else {
            vec![Dataset::default()
                .name("csv")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Scatter)
                .style(Style::default().cyan())
                .data(&main_series.points)]
        };

        // Create the X axis and define its properties
        let x_axis = Axis::default()
            .title(args.arg_x.red())
            .style(Style::default().white())
            .bounds(main_series.x_domain().unwrap())
            .labels(main_series.x_graduations().unwrap());

        // Create the Y axis and define its properties
        let y_axis = Axis::default()
            .title(args.arg_y.red())
            .style(Style::default().white())
            .bounds(main_series.y_domain().unwrap())
            .labels(main_series.y_graduations().unwrap());

        // Create the chart and link all the parts together
        let chart = Chart::new(datasets)
            .block(Block::default())
            .x_axis(x_axis)
            .y_axis(y_axis)
            .legend_position(if showing_multiple_series {
                Some(LegendPosition::TopRight)
            } else {
                None
            });

        frame.render_widget(chart, area);
    })?;

    let contents = &terminal.backend().buffer().content;

    let mut i: usize = 0;

    fn group_cells_by_color(cells: &[Cell]) -> Vec<Vec<Cell>> {
        let mut groups: Vec<Vec<Cell>> = Vec::new();
        let mut current_run: Vec<Cell> = Vec::new();

        for cell in cells {
            if current_run.is_empty() || current_run[0].fg == cell.fg {
                current_run.push(cell.clone());
                continue;
            }

            groups.push(current_run);

            current_run = Vec::new();
            current_run.push(cell.clone());
        }

        if !current_run.is_empty() {
            groups.push(current_run);
        }

        groups
    }

    fn colorize(string: &str, color: Color) -> ColoredString {
        match color {
            Color::Reset | Color::White => Colorize::normal(string),
            Color::Red => Colorize::red(string),
            Color::Blue => Colorize::blue(string),
            Color::Cyan => Colorize::cyan(string),
            _ => {
                dbg!(&color);
                unimplemented!();
            }
        }
    }

    while i < contents.len() {
        let line = group_cells_by_color(&contents[i..(i + cols as usize)])
            .iter()
            .map(|cells| {
                colorize(
                    &cells.iter().map(|cell| cell.symbol()).collect::<String>(),
                    cells[0].fg,
                )
                .to_string()
            })
            .collect::<String>();

        println!("{}", line);

        i += cols as usize;
    }

    Ok(())
}

struct Series {
    points: Vec<(f64, f64)>,
    extent: Option<((f64, f64), (f64, f64))>,
}

impl Series {
    fn new() -> Self {
        Self {
            points: Vec::new(),
            extent: None,
        }
    }

    fn of(x: f64, y: f64) -> Self {
        Self {
            points: vec![(x, y)],
            extent: Some(((x, x), (y, y))),
        }
    }

    fn add(&mut self, x: f64, y: f64) {
        self.points.push((x, y));

        match self.extent.as_mut() {
            None => self.extent = Some(((x, x), (y, y))),
            Some(((x_min, x_max), (y_min, y_max))) => {
                if x < *x_min {
                    *x_min = x;
                }
                if x > *x_max {
                    *x_max = x;
                }

                if y < *y_min {
                    *y_min = y;
                }
                if y > *y_max {
                    *y_max = y;
                }
            }
        };
    }

    fn x_domain(&self) -> Option<[f64; 2]> {
        self.extent.map(|((min, max), _)| [min, max])
    }

    fn y_domain(&self) -> Option<[f64; 2]> {
        self.extent.map(|(_, (min, max))| [min, max])
    }

    fn x_graduations(&self) -> Option<Vec<Span>> {
        self.extent
            .map(|((min, max), _)| graduations_from_domain(min, max))
    }

    fn y_graduations(&self) -> Option<Vec<Span>> {
        self.extent
            .map(|(_, (min, max))| graduations_from_domain(min, max))
    }
}
