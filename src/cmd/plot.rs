use std::collections::HashMap;
use std::num::NonZeroUsize;

use colored::{ColoredString, Colorize};
use serde::de::{Deserialize, Deserializer, Error};

use ratatui::backend::TestBackend;
use ratatui::buffer::Cell;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::Terminal;

use crate::config::{Config, Delimiter};
use crate::moonblade::DynamicNumber;
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

fn lerp(min: f64, max: f64, t: f64) -> f64 {
    (1.0 - t) * min + t * max
}

fn format_graduation(formatter: &mut numfmt::Formatter, axis_type: AxisType, x: f64) -> String {
    util::pretty_print_float(
        formatter,
        match axis_type {
            AxisType::Float => x,
            AxisType::Int => x.trunc(),
        },
    )
}

fn graduations_from_domain(
    formatter: &mut numfmt::Formatter,
    axis_type: AxisType,
    domain: (DynamicNumber, DynamicNumber),
    steps: usize,
) -> Vec<String> {
    debug_assert!(steps > 1);

    let mut graduations: Vec<String> = Vec::with_capacity(steps);

    let mut t = 0.0;
    let fract = 1.0 / (steps - 1) as f64;

    graduations.push(format_graduation(formatter, axis_type, domain.0.as_float()));

    for _ in 1..(steps - 1) {
        t += fract;
        graduations.push(format_graduation(
            formatter,
            axis_type,
            lerp(domain.0.as_float(), domain.1.as_float(), t),
        ));
    }

    graduations.push(format_graduation(formatter, axis_type, domain.1.as_float()));

    graduations
}

fn merge_domains(
    mut domains: impl Iterator<Item = (DynamicNumber, DynamicNumber)>,
) -> (DynamicNumber, DynamicNumber) {
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

fn merge_axis_types(mut axis_types: impl Iterator<Item = AxisType>) -> AxisType {
    let mut axis_type = axis_types.next().unwrap();

    for other in axis_types {
        axis_type = axis_type.and(other);
    }

    axis_type
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
    -L, --line           Whether to draw a line plot instead of a scatter plot.
    --color <column>     Name of the categorical column that will be used to
                         color the different points.
    --cols <num>         Width of the graph in terminal columns, i.e. characters.
                         Defaults to using all your terminal's width or 80 if
                         terminal size cannot be found (i.e. when piping to file).
    --rows <num>         Height of the graph in terminal rows, i.e. characters.
                         Defaults to using all your terminal's height minus 2 or 30 if
                         terminal size cannot be found (i.e. when piping to file).
    -M, --marker <name>  Marker to use. Can be one of (by order of size): 'braille', 'dot',
                         'halfblock', 'bar', 'block'.
                         [default: braille]
    --x-ticks <n>        Number of x-axis graduation steps.
                         [default: 3]
    --y-ticks <n>        Number of y-axis graduation steps.
                         [default: 4]
    --x-min <n>          Force a minimum value for x axis.
    --x-max <n>          Force a maximum value for x axis.
    --y-min <n>          Force a minimum value for y axis.
    --y-max <n>          Force a maximum value for y axis.

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
    flag_x_ticks: NonZeroUsize,
    flag_y_ticks: NonZeroUsize,
    flag_x_min: Option<DynamicNumber>,
    flag_x_max: Option<DynamicNumber>,
    flag_y_min: Option<DynamicNumber>,
    flag_y_max: Option<DynamicNumber>,
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

        // Filtering out-of-bounds values
        if matches!(args.flag_x_min, Some(x_min) if x < x_min)
            || matches!(args.flag_x_max, Some(x_max) if x > x_max)
            || matches!(args.flag_y_min, Some(y_min) if y < y_min)
            || matches!(args.flag_y_max, Some(y_max) if y > y_max)
        {
            continue;
        }

        if let Some(i) = color_column_index {
            grouped_series.add_with_name(&record[i], x, y)
        } else {
            grouped_series.add(x, y);
        }
    }

    let mut finalized_series = grouped_series.finalize();

    for (_, series) in finalized_series.iter_mut() {
        // Domain bounds
        if let Some(x_min) = args.flag_x_min {
            series.set_x_min(x_min);
        }
        if let Some(x_max) = args.flag_x_max {
            series.set_x_max(x_max);
        }
        if let Some(y_min) = args.flag_y_min {
            series.set_y_min(y_min);
        }
        if let Some(y_max) = args.flag_y_max {
            series.set_y_max(y_max);
        }

        // NOTE: we sort on x if we want a line plot
        if args.flag_line {
            series.sort_by_x_axis();
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

        let n = finalized_series[0].1.len();

        // x axis information
        let x_domain = merge_domains(
            finalized_series
                .iter()
                .map(|(_, series)| series.x_domain().unwrap()),
        );
        let x_axis_type =
            merge_axis_types(finalized_series.iter().map(|(_, series)| series.types.0));
        let can_display_x_axis_title = finalized_series
            .iter()
            .all(|(_, series)| series.can_display_x_axis_title());

        // y axis information
        let y_domain = merge_domains(
            finalized_series
                .iter()
                .map(|(_, series)| series.y_domain().unwrap()),
        );
        let y_axis_type =
            merge_axis_types(finalized_series.iter().map(|(_, series)| series.types.1));
        let can_display_y_axis_title = finalized_series
            .iter()
            .all(|(_, series)| series.can_display_y_axis_title());

        // Create the datasets to fill the chart with
        let finalized_series = finalized_series
            .into_iter()
            .map(|(name_opt, series)| (name_opt, series.into_floats()))
            .collect::<Vec<_>>();

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
                    .data(series);

                if let Some(name) = name_opt {
                    dataset = dataset.name(name.clone());
                }

                dataset
            })
            .collect();

        let mut formatter = util::acquire_number_formatter();

        // Create the X axis and define its properties
        let x_axis = Axis::default()
            .title(if can_display_x_axis_title {
                args.arg_x.dim()
            } else {
                "".dim()
            })
            .style(Style::default().white())
            .bounds([x_domain.0.as_float(), x_domain.1.as_float()])
            .labels(graduations_from_domain(
                &mut formatter,
                x_axis_type,
                x_domain,
                args.flag_x_ticks.get().min(n.max(2)),
            ));

        // Create the Y axis and define its properties
        let y_axis = Axis::default()
            .title(if can_display_y_axis_title {
                args.arg_y.dim()
            } else {
                "".dim()
            })
            .style(Style::default().white())
            .bounds([y_domain.0.as_float(), y_domain.1.as_float()])
            .labels(graduations_from_domain(
                &mut formatter,
                y_axis_type,
                y_domain,
                args.flag_y_ticks.get().min(n.max(2)),
            ));

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

#[derive(Debug, Clone, Copy)]
enum AxisType {
    Int,
    Float,
}

impl AxisType {
    fn and(self, other: AxisType) -> Self {
        match (self, other) {
            (Self::Float, _) | (_, Self::Float) => Self::Float,
            _ => Self::Int,
        }
    }
}

struct Series {
    types: (AxisType, AxisType),
    points: Vec<(DynamicNumber, DynamicNumber)>,
    extent: Option<(
        (DynamicNumber, DynamicNumber),
        (DynamicNumber, DynamicNumber),
    )>,
}

impl Series {
    fn new() -> Self {
        Self {
            types: (AxisType::Int, AxisType::Int),
            points: Vec::new(),
            extent: None,
        }
    }

    fn len(&self) -> usize {
        self.points.len()
    }

    fn into_floats(self) -> Vec<(f64, f64)> {
        self.points
            .into_iter()
            .map(|(x, y)| (x.as_float(), y.as_float()))
            .collect()
    }

    fn of(x: DynamicNumber, y: DynamicNumber) -> Self {
        let mut series = Self::new();
        series.add(x, y);
        series
    }

    fn add(&mut self, x: DynamicNumber, y: DynamicNumber) {
        if x.is_float() {
            self.types.0 = AxisType::Float;
        }

        if y.is_float() {
            self.types.1 = AxisType::Float;
        }

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

    fn x_domain(&self) -> Option<(DynamicNumber, DynamicNumber)> {
        self.extent.map(|(x, _)| x)
    }

    fn y_domain(&self) -> Option<(DynamicNumber, DynamicNumber)> {
        self.extent.map(|(_, y)| y)
    }

    fn can_display_x_axis_title(&self) -> bool {
        if let Some((extent_x, extent_y)) = self.extent {
            let max_x = extent_x.1;
            let min_y = extent_y.0;

            !self.points.iter().any(|(x, y)| *x == max_x && *y == min_y)
        } else {
            true
        }
    }

    fn can_display_y_axis_title(&self) -> bool {
        if let Some((extent_x, extent_y)) = self.extent {
            let min_x = extent_x.0;
            let max_y = extent_y.1;

            !self.points.iter().any(|(x, y)| *x == min_x && *y == max_y)
        } else {
            true
        }
    }

    fn set_x_min(&mut self, v: DynamicNumber) {
        if let Some((x, _)) = self.extent.as_mut() {
            x.0 = v;
        }
    }

    fn set_x_max(&mut self, v: DynamicNumber) {
        if let Some((x, _)) = self.extent.as_mut() {
            x.1 = v;
        }
    }

    fn set_y_min(&mut self, v: DynamicNumber) {
        if let Some((_, y)) = self.extent.as_mut() {
            y.0 = v;
        }
    }

    fn set_y_max(&mut self, v: DynamicNumber) {
        if let Some((_, y)) = self.extent.as_mut() {
            y.1 = v;
        }
    }

    fn sort_by_x_axis(&mut self) {
        self.points.sort_by(|a, b| a.0.cmp(&b.0))
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

    fn add(&mut self, x: DynamicNumber, y: DynamicNumber) {
        self.default.as_mut().unwrap().add(x, y);
    }

    fn add_with_name(&mut self, name: &[u8], x: DynamicNumber, y: DynamicNumber) {
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
