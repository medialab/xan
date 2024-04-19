use std::collections::HashMap;

use colored::{ColoredString, Colorize};
use serde::de::{Deserialize, Deserializer, Error};

use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols;
use ratatui::text::Span;
// use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Padding};
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::Terminal;

use crate::config::{Config, Delimiter};
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

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

fn graduations_from_domain<'a>(domain: (f64, f64)) -> Vec<Span<'a>> {
    vec![
        Span::from(domain.0.to_string()),
        Span::from(domain.1.to_string()),
    ]
}

fn merge_domains(mut domains: impl Iterator<Item = (f64, f64)>) -> (f64, f64) {
    let mut domain = domains.next().unwrap();

    for other in domains {
        if other.0 < domain.0 {
            domain.0 = other.0;
        }
        if other.1 > domain.1 {
            domain.1 = other.1;
        }
    }

    domain
}

#[derive(Clone, Copy)]
struct Marker(symbols::Marker);

impl Marker {
    fn into_inner(self) -> symbols::Marker {
        self.0
    }
}

impl<'de> Deserialize<'de> for Marker {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;

        Ok(Marker(match raw.as_str() {
            "dot" => symbols::Marker::Dot,
            "braille" => symbols::Marker::Braille,
            "halfblock" => symbols::Marker::HalfBlock,
            "block" => symbols::Marker::Block,
            "bar" => symbols::Marker::Bar,
            _ => return Err(D::Error::custom(format!("invalid marker type \"{}\"", raw))),
        }))
    }
}

static USAGE: &str = "
Draw a scatter plot or a line plot based on 2-dimensional data.

Usage:
    xsv plot [options] <x> <y> [<input>]
    xsv plot --help

plot options:
    --line            Whether to draw a line plot instead of a scatter plot.
    --color <column>  Name of the categorical column that will be used to
                      color the different points.
    --cols <num>      Width of the graph in terminal columns, i.e. characters.
                      Defaults to using all your terminal's width or 80 if
                      terminal size cannot be found (i.e. when piping to file).
    --rows <num>      Height of the graph in terminal rows, i.e. characters.
                      Defaults to using all your terminal's height minus 2 or 30 if
                      terminal size cannot be found (i.e. when piping to file).
    --marker <name>   Marker to use. Can be one of (by order of size): 'braille', 'dot',
                      'halfblock', 'bar', 'block'.
                      [default: braille]

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
    flag_line: bool,
    flag_cols: Option<usize>,
    flag_rows: Option<usize>,
    flag_color: Option<String>,
    flag_marker: Marker,
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

    let mut grouped_series = if showing_multiple_series {
        GroupedSeries::with_groups()
    } else {
        GroupedSeries::default()
    };

    while rdr.read_byte_record(&mut record)? {
        let x = String::from_utf8_lossy(&record[x_column_index])
            .parse()
            .expect("could not parse number");
        let y = String::from_utf8_lossy(&record[y_column_index])
            .parse()
            .expect("could not parse number");

        if let Some(i) = color_column_index {
            grouped_series.add_with_name(&record[i], x, y)
        } else {
            grouped_series.add(x, y);
        }
    }

    let mut finalized_series = grouped_series.finalize();

    // NOTE: we sort on x if we want a line plot
    if args.flag_line {
        for (_, series) in finalized_series.iter_mut() {
            series.sort();
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

        let x_domain = merge_domains(
            finalized_series
                .iter()
                .map(|(_, series)| series.x_domain().unwrap()),
        );

        let y_domain = merge_domains(
            finalized_series
                .iter()
                .map(|(_, series)| series.y_domain().unwrap()),
        );

        let datasets = finalized_series
            .iter()
            .enumerate()
            .map(|(i, (name_opt, series))| {
                let mut dataset = Dataset::default()
                    .marker(args.flag_marker.into_inner())
                    .graph_type(if args.flag_line {
                        GraphType::Line
                    } else {
                        GraphType::Scatter
                    })
                    .style(get_series_color(i))
                    .data(&series.points);

                if let Some(name) = name_opt {
                    dataset = dataset.name(name.clone());
                }

                dataset
            })
            .collect();

        // Create the X axis and define its properties
        let x_axis = Axis::default()
            .title(args.arg_x.dim())
            .style(Style::default().white())
            .bounds([x_domain.0, x_domain.1])
            .labels(graduations_from_domain(x_domain));

        // Create the Y axis and define its properties
        let y_axis = Axis::default()
            .title(args.arg_y.dim())
            .style(Style::default().white())
            .bounds([y_domain.0, y_domain.1])
            .labels(graduations_from_domain(y_domain));

        // Create the chart and link all the parts together
        let mut chart = Chart::new(datasets)
            // .block(
            //     Block::default()
            //         .border_style(Style::default().dim())
            //         .borders(Borders::ALL)
            //         .padding(Padding::symmetric(2, 1)),
            // )
            .x_axis(x_axis)
            .y_axis(y_axis);

        if !showing_multiple_series {
            chart = chart.legend_position(None);
        } else {
            chart = chart.hidden_legend_constraints((Constraint::Min(0), Constraint::Min(0)));
        }

        frame.render_widget(chart, area);
    })?;

    let contents = &terminal.backend().buffer().content;

    let mut i: usize = 0;

    fn group_cells_by_color(cells: &[Cell]) -> Vec<Vec<Cell>> {
        let mut groups: Vec<Vec<Cell>> = Vec::new();
        let mut current_run: Vec<Cell> = Vec::new();

        for cell in cells {
            if current_run.is_empty() || (current_run[0].style() == cell.style()) {
                current_run.push(cell.clone());
                continue;
            }

            groups.push(current_run);

            current_run = vec![cell.clone()];
        }

        if !current_run.is_empty() {
            groups.push(current_run);
        }

        groups
    }

    fn colorize(string: &str, color: Color, modifer: Modifier) -> ColoredString {
        let string = match color {
            Color::Reset | Color::White => Colorize::normal(string),
            Color::Red => Colorize::red(string),
            Color::Blue => Colorize::blue(string),
            Color::Cyan => Colorize::cyan(string),
            Color::Green => Colorize::green(string),
            Color::Yellow => Colorize::yellow(string),
            Color::Magenta => Colorize::magenta(string),
            _ => {
                dbg!(&color);
                unimplemented!();
            }
        };

        if modifer.is_empty() {
            return string;
        }

        match modifer {
            Modifier::DIM => Colorize::dimmed(string),
            _ => {
                dbg!(&modifer);
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
                    cells[0].modifier,
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

    fn x_domain(&self) -> Option<(f64, f64)> {
        self.extent.map(|(x, _)| x)
    }

    fn y_domain(&self) -> Option<(f64, f64)> {
        self.extent.map(|(_, y)| y)
    }

    fn sort(&mut self) {
        self.points.sort_by(|a, b| a.0.total_cmp(&b.0))
    }
}

struct GroupedSeries {
    mapping: Option<HashMap<Vec<u8>, (usize, Series)>>,
    default: Option<Series>,
}

impl Default for GroupedSeries {
    fn default() -> Self {
        Self {
            mapping: None,
            default: Some(Series::new()),
        }
    }
}

impl GroupedSeries {
    fn with_groups() -> Self {
        Self {
            mapping: Some(HashMap::new()),
            default: None,
        }
    }

    fn add(&mut self, x: f64, y: f64) {
        self.default.as_mut().unwrap().add(x, y);
    }

    fn add_with_name(&mut self, name: &[u8], x: f64, y: f64) {
        let mapping = self.mapping.as_mut().unwrap();
        let current_len = mapping.len();

        mapping
            .entry(name.to_vec())
            .and_modify(|(_, series)| {
                series.add(x, y);
            })
            .or_insert_with(|| (current_len, Series::of(x, y)));
    }

    fn finalize(self) -> Vec<(Option<String>, Series)> {
        let mut output = Vec::new();

        if let Some(default_series) = self.default {
            output.push((None, default_series));
        } else if let Some(mapping) = self.mapping {
            let mut items = mapping.into_iter().collect::<Vec<_>>();
            items.sort_by(|a, b| a.1 .0.cmp(&b.1 .0));

            for (name, (_, series)) in items {
                output.push((Some(String::from_utf8(name).unwrap()), series));
            }
        }

        output
    }
}
