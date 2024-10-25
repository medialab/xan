// Issue tracking:
//  - https://github.com/ratatui/ratatui/issues/334
//  - https://github.com/ratatui/ratatui/issues/1391

use std::collections::HashMap;
use std::num::NonZeroUsize;

use colored::{ColoredString, Colorize};
use indexmap::IndexMap;
use jiff::{
    civil::{Date, DateTime, Time},
    tz::TimeZone,
    Timestamp, Unit, Zoned, ZonedRound,
};
use serde::de::{Deserialize, Deserializer, Error};
use unicode_width::UnicodeWidthStr;

use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};
use ratatui::Terminal;

use crate::config::{Config, Delimiter};
use crate::moonblade::DynamicNumber;
use crate::select::SelectColumns;
use crate::util;
use crate::{CliError, CliResult};

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

        Ok(Self(match raw.as_str() {
            "dot" => symbols::Marker::Dot,
            "braille" => symbols::Marker::Braille,
            "halfblock" => symbols::Marker::HalfBlock,
            "block" => symbols::Marker::Block,
            "bar" => symbols::Marker::Bar,
            _ => {
                return Err(D::Error::custom(format!(
                    "unknown marker type \"{}\"!",
                    raw
                )))
            }
        }))
    }
}

#[derive(Clone, Copy)]
struct Granularity(Unit);

impl Granularity {
    fn into_inner(self) -> Unit {
        self.0
    }
}

impl<'de> Deserialize<'de> for Granularity {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;

        Ok(Self(match raw.as_str() {
            "year" | "years" => Unit::Year,
            "month" | "months" => Unit::Month,
            "day" | "days" => Unit::Day,
            "hour" | "hours" => Unit::Hour,
            "minute" | "minutes" => Unit::Minute,
            "second" | "seconds" => Unit::Second,
            _ => {
                return Err(D::Error::custom(format!(
                    "invalid granularity \"{}\"!",
                    raw
                )))
            }
        }))
    }
}

static USAGE: &str = "
Draw a scatter plot or a line plot based on 2-dimensional data.

Usage:
    xan plot --count [options] <x> [<input>]
    xan plot [options] <x> <y> [<input>]
    xan plot --help

plot options:
    -L, --line                 Whether to draw a line plot instead of the default scatter plot.
    -B, --bars                 Whether to draw bars instead of the default scatter plot.
                               WARNING: currently does not work if y range does not include 0.
                               (https://github.com/ratatui/ratatui/issues/1391)
    -T, --time                 Use to indicate that the x axis is temporal. The axis will be
                               discretized according to some inferred temporal granularity and
                               y values will be summed wrt the newly discretized x axis.
    --count                    Omit the y column and count rows instead. Only relevant when
                               used with -T, --time that will discretize the x axis.
    -C, --category <col>       Name of the categorical column that will be used to
                               draw distinct series per category.
                               Incompatible with -Y, --add-series.
    -Y, --add-series <col>     Name of another column of y values to add as a new series.
                               Incompatible with -C, --category.
    -g, --granularity <g>      Force temporal granularity for x axis discretization when
                               using -T, --time. Must be one of \"years\", \"months\", \"days\",
                               \"hours\", \"minutes\" or \"seconds\". Will be inferred if omitted.
    --cols <num>               Width of the graph in terminal columns, i.e. characters.
                               Defaults to using all your terminal's width or 80 if
                               terminal size cannot be found (i.e. when piping to file).
                               Can also be given as a ratio of the terminal's width e.g. \"0.5\".
    --rows <num>               Height of the graph in terminal rows, i.e. characters.
                               Defaults to using all your terminal's height minus 2 or 30 if
                               terminal size cannot be found (i.e. when piping to file).
                               Can also be given as a ratio of the terminal's height e.g. \"0.5\".
    -S, --small-multiples <n>  Display small multiples of datasets given by -C, --category
                               or -Y, --add-series using the provided number of grid columns.
                               The plot will all share the same x scale but use a different y scale by
                               default. See --share-y-scale and --separate-x-scale to tweak this behavior.
    --share-x-scale <yes|no>   Give \"yes\" to share x scale for all plot when drawing small multiples with -S,
                               or \"no\" to keep them separate.
                               [default: yes]
    --share-y-scale <yes|no>   Give \"yes\" to share y scale for all plot when drawing small multiples with -S,
                               or \"no\" to keep them separate. Defaults to \"yes\" when -C, --category is given
                               and \"no\" when -Y, --add-series is given.
    -M, --marker <name>        Marker to use. Can be one of (by order of size): 'braille', 'dot',
                               'halfblock', 'bar', 'block'.
                               [default: braille]
    -G, --grid                 Draw a background grid.
    --x-ticks <n>              Number of x-axis graduation steps. Will default to some sensible number based on
                               the dimensions of the terminal.
    --y-ticks <n>              Number of y-axis graduation steps. Will default to some sensible number based on
                               the dimensions of the terminal.
    --x-min <n>                Force a minimum value for the x axis.
    --x-max <n>                Force a maximum value for the x axis.
    --y-min <n>                Force a minimum value for the y axis.
    --y-max <n>                Force a maximum value for the y axis.

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
    arg_x: SelectColumns,
    arg_y: Option<SelectColumns>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_line: bool,
    flag_bars: bool,
    flag_time: bool,
    flag_count: bool,
    flag_cols: Option<String>,
    flag_rows: Option<String>,
    flag_small_multiples: Option<NonZeroUsize>,
    flag_share_x_scale: String,
    flag_share_y_scale: Option<String>,
    flag_category: Option<SelectColumns>,
    flag_add_series: Vec<SelectColumns>,
    flag_marker: Marker,
    flag_granularity: Option<Granularity>,
    flag_grid: bool,
    flag_x_ticks: Option<NonZeroUsize>,
    flag_y_ticks: Option<NonZeroUsize>,
    flag_x_min: Option<String>,
    flag_x_max: Option<String>,
    flag_y_min: Option<DynamicNumber>,
    flag_y_max: Option<DynamicNumber>,
}

impl Args {
    fn parse_x_bounds(&self) -> CliResult<(Option<DynamicNumber>, Option<DynamicNumber>)> {
        if self.flag_time {
            Ok((
                self.flag_x_min
                    .as_ref()
                    .map(|cell| parse_as_timestamp(cell.as_bytes()))
                    .transpose()?,
                self.flag_x_max
                    .as_ref()
                    .map(|cell| parse_as_timestamp(cell.as_bytes()))
                    .transpose()?,
            ))
        } else {
            Ok((
                self.flag_x_min
                    .as_ref()
                    .map(|cell| parse_as_number(cell.as_bytes()))
                    .transpose()?,
                self.flag_x_max
                    .as_ref()
                    .map(|cell| parse_as_number(cell.as_bytes()))
                    .transpose()?,
            ))
        }
    }

    fn infer_y_ticks(&self, rows: usize) -> usize {
        let ideal_y_ticks = {
            let y_axis_rows = rows.saturating_sub(2);

            // NOTE: those shenanigans is to try and find some division
            // that will not split the grid into uneven steps
            if y_axis_rows % 5 == 0 {
                y_axis_rows / 5
            } else if y_axis_rows % 4 == 0 {
                y_axis_rows / 4
            } else if y_axis_rows % 6 == 0 {
                y_axis_rows / 6
            } else if y_axis_rows % 3 == 0 {
                y_axis_rows / 3
            } else {
                y_axis_rows / 5
            }
        };

        self.flag_y_ticks
            .map(|n| n.get())
            .unwrap_or(ideal_y_ticks.max(3))
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let share_x_scale = args.flag_share_x_scale == "yes";
    let share_y_scale = args
        .flag_share_y_scale
        .as_ref()
        .map(|choice| choice == "yes")
        .unwrap_or(args.flag_add_series.is_empty());

    debug_assert!(if args.flag_count {
        args.arg_y.is_none()
    } else {
        true
    });

    let (flag_x_min, flag_x_max) = args.parse_x_bounds()?;

    if args.flag_category.is_some() && !args.flag_add_series.is_empty() {
        Err("-C, --category cannot work with -Y, --add-series!")?;
    }

    if matches!(args.flag_x_ticks, Some(n) if n.get() < 2) {
        Err("--x-ticks must be > 1!")?;
    }

    if matches!(args.flag_y_ticks, Some(n) if n.get() < 2) {
        Err("--y-ticks must be > 1!")?;
    }

    let has_added_series = !args.flag_add_series.is_empty();

    // Collecting data
    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?;

    let x_column_index = Config::new(&None)
        .select(args.arg_x.clone())
        .single_selection(headers)?;

    let y_column_index_opt = args
        .arg_y
        .as_ref()
        .map(|name| {
            Config::new(&None)
                .select(name.clone())
                .single_selection(headers)
        })
        .transpose()?;

    let x_column_name = if args.flag_no_headers {
        x_column_index.to_string()
    } else {
        std::str::from_utf8(&headers[x_column_index])
            .unwrap()
            .to_string()
    };

    let y_column_name = match y_column_index_opt {
        Some(y_column_index) => {
            if args.flag_no_headers {
                y_column_index.to_string()
            } else {
                std::str::from_utf8(&headers[y_column_index])
                    .unwrap()
                    .to_string()
            }
        }
        None => "count()".to_string(),
    };

    let category_column_index = args
        .flag_category
        .as_ref()
        .map(|name| {
            Config::new(&None)
                .select(name.clone())
                .single_selection(headers)
        })
        .transpose()?;

    let additional_series_indices = args
        .flag_add_series
        .iter()
        .map(|name| {
            let i = Config::new(&None)
                .select(name.clone())
                .single_selection(headers)?;

            let col_name = if args.flag_no_headers {
                i.to_string().into_bytes()
            } else {
                headers[i].to_vec()
            };

            Ok((col_name, i))
        })
        .collect::<Result<Vec<(Vec<u8>, usize)>, CliError>>()?;

    let showing_multiple_series =
        category_column_index.is_some() || !additional_series_indices.is_empty();

    let mut record = csv::ByteRecord::new();

    let mut series_builder = if !additional_series_indices.is_empty() {
        let mut multiple_series = MultipleSeries::with_capacity(additional_series_indices.len());
        multiple_series.register_series(y_column_name.as_bytes());

        for (name, _) in additional_series_indices.iter() {
            multiple_series.register_series(name);
        }

        SeriesBuilder::Multiple(multiple_series)
    } else if category_column_index.is_some() {
        SeriesBuilder::new_categorical()
    } else {
        SeriesBuilder::new_single()
    };

    while rdr.read_byte_record(&mut record)? {
        let x_cell = &record[x_column_index];

        let x = if args.flag_time {
            parse_as_timestamp(x_cell)?
        } else {
            parse_as_number(x_cell)?
        };

        let y = match y_column_index_opt {
            Some(y_column_index) => parse_as_number(&record[y_column_index])?,
            None => DynamicNumber::Integer(1),
        };

        // Filtering out-of-bounds values
        if matches!(flag_x_min, Some(x_min) if x < x_min)
            || matches!(flag_x_max, Some(x_max) if x > x_max)
            || matches!(args.flag_y_min, Some(y_min) if y < y_min)
            || matches!(args.flag_y_max, Some(y_max) if y > y_max)
        {
            continue;
        }

        if let Some(i) = category_column_index {
            series_builder.add_with_name(record[i].to_vec(), x, y)
        } else if !additional_series_indices.is_empty() {
            series_builder.add_with_index(0, x, y);

            for (i, (_, pos)) in additional_series_indices.iter().enumerate() {
                let v = parse_as_number(&record[*pos])?;

                series_builder.add_with_index(i + 1, x, v);
            }
        } else {
            series_builder.add(x, y);
        }
    }

    if series_builder.is_empty() {
        println!("Nothing to display!");
        return Ok(());
    }

    let mut finalized_series = series_builder.into_finalized_series();

    for (_, series) in finalized_series.iter_mut() {
        if args.flag_time {
            series.mark_as_temporal(args.flag_granularity.map(|g| g.into_inner()));
        }

        // Domain bounds
        if let Some(x_min) = flag_x_min {
            series.set_x_min(x_min);
        }
        if let Some(x_max) = flag_x_max {
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

    // Solving cols & rows
    let mut cols = util::acquire_term_cols(&None);

    if let Some(spec) = &args.flag_cols {
        if spec.contains('.') {
            let ratio = spec.parse::<f64>().map_err(|_| "--cols is invalid! ")?;

            cols = (cols as f64 * ratio).trunc().abs() as usize;
        } else {
            cols = spec.parse::<usize>().map_err(|_| "--cols is invalid! ")?;
        }
    }

    if cols < 10 {
        Err("not enough cols to draw!")?;
    }

    let mut rows = util::acquire_term_rows().unwrap_or(30);

    if let Some(spec) = &args.flag_rows {
        if spec.contains('.') {
            let ratio = spec.parse::<f64>().map_err(|_| "--rows is invalid! ")?;

            rows = (rows as f64 * ratio).trunc().abs() as usize;
        } else {
            rows = spec.parse::<usize>().map_err(|_| "--rows is invalid! ")?;
        }
    }

    if rows < 3 {
        Err("not enough rows to draw!")?;
    }

    // NOTE: when drawing small multiples, if --rows was not given, we split vertical space
    // if we have more than what can fit in a single column by default
    if let Some(grid_cols) = args.flag_small_multiples {
        if args.flag_rows.is_none() && finalized_series.windows(grid_cols.get()).count() > 1 {
            rows /= 2;
        }
    }

    // NOTE: leaving one row for the prompt
    rows = rows.saturating_sub(1);

    let y_ticks = args.infer_y_ticks(rows);

    // Drawing
    let mut terminal = Terminal::new(TestBackend::new(cols as u16, rows as u16))?;

    match args.flag_small_multiples {
        None => {
            terminal.draw(|frame| {
                // let n = finalized_series[0].1.len();

                // x axis information
                let (x_axis_info, y_axis_info) =
                    AxisInfo::from_multiple_series(finalized_series.iter());

                // Create the datasets to fill the chart with
                let finalized_series = finalized_series
                    .iter()
                    .map(|(name_opt, series)| (name_opt, series.to_floats()))
                    .collect::<Vec<_>>();

                let datasets = finalized_series
                    .iter()
                    .enumerate()
                    .map(|(i, (name_opt, series))| {
                        let mut dataset = Dataset::default()
                            .marker(args.flag_marker.into_inner())
                            .graph_type(if args.flag_line {
                                GraphType::Line
                            } else if args.flag_bars {
                                GraphType::Bar
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

                // Create the Y axis and define its properties
                let y_ticks_labels = graduations_from_domain(
                    &mut formatter,
                    y_axis_info.axis_type,
                    y_axis_info.domain,
                    y_ticks,
                );
                let x_ticks = infer_x_ticks(
                    args.flag_x_ticks,
                    &mut formatter,
                    &x_axis_info,
                    &y_ticks_labels,
                    cols,
                );
                let y_axis = Axis::default()
                    .title(if !has_added_series && y_axis_info.can_be_displayed {
                        y_column_name.dim()
                    } else {
                        "".dim()
                    })
                    .style(Style::default().white())
                    .bounds([
                        y_axis_info.domain.0.as_float(),
                        y_axis_info.domain.1.as_float(),
                    ])
                    .labels(y_ticks_labels);

                // Create the X axis and define its properties
                let x_ticks_labels = graduations_from_domain(
                    &mut formatter,
                    x_axis_info.axis_type,
                    x_axis_info.domain,
                    x_ticks,
                );
                let x_axis = Axis::default()
                    .title(if x_axis_info.can_be_displayed {
                        x_column_name.dim()
                    } else {
                        "".dim()
                    })
                    .style(Style::default().white())
                    .bounds([
                        x_axis_info.domain.0.as_float(),
                        x_axis_info.domain.1.as_float(),
                    ])
                    .labels(x_ticks_labels.clone());

                // Create the chart and link all the parts together
                let mut chart = Chart::new(datasets).x_axis(x_axis).y_axis(y_axis);

                if !showing_multiple_series {
                    chart = chart.legend_position(None);
                } else {
                    chart =
                        chart.hidden_legend_constraints((Constraint::Min(0), Constraint::Min(0)));
                }

                frame.render_widget(chart, frame.area());
                patch_buffer(frame.buffer_mut(), None, &x_ticks_labels, args.flag_grid);
            })?;

            print_terminal(&terminal, cols);
        }
        Some(grid_cols) => {
            let grid_cols = grid_cols.get();
            let mut color_i: usize = 0;
            let mut first_grid_col = true;

            let (harmonized_x_axis_info, harmonized_y_axis_info) =
                AxisInfo::from_multiple_series(finalized_series.iter());

            for finalized_series_column in finalized_series.chunks(grid_cols) {
                let x_column_name = x_column_name.clone();

                if first_grid_col {
                    first_grid_col = false;
                } else {
                    println!();
                }

                terminal.draw(|frame| {
                    let actual_grid_cols = finalized_series_column.len();

                    let layout = Layout::default()
                        .direction(Direction::Horizontal)
                        .spacing(2)
                        .constraints(vec![
                            Constraint::Ratio(1, grid_cols as u32);
                            actual_grid_cols
                        ])
                        .split(frame.area());

                    for (i, single_finalized_series) in finalized_series_column.iter().enumerate() {
                        // let n = single_finalized_series.1.len();

                        // x axis information
                        let (mut x_axis_info, mut y_axis_info) =
                            AxisInfo::from_single_series(single_finalized_series);

                        if share_x_scale {
                            x_axis_info = harmonized_x_axis_info.clone();
                        }

                        if share_y_scale {
                            y_axis_info = harmonized_y_axis_info.clone();
                        }

                        // Create the datasets to fill the chart with
                        let single_finalized_series = (
                            single_finalized_series.0.clone(),
                            single_finalized_series.1.to_floats(),
                        );

                        let mut dataset = Dataset::default()
                            .marker(args.flag_marker.into_inner())
                            .graph_type(if args.flag_line {
                                GraphType::Line
                            } else if args.flag_bars {
                                GraphType::Bar
                            } else {
                                GraphType::Scatter
                            })
                            .style(get_series_color(color_i))
                            .data(&single_finalized_series.1);

                        if let Some(name) = &single_finalized_series.0 {
                            dataset = dataset.name(name.clone());
                        }

                        let mut formatter = util::acquire_number_formatter();

                        // Create the Y axis and define its properties
                        let y_ticks_labels = graduations_from_domain(
                            &mut formatter,
                            y_axis_info.axis_type,
                            y_axis_info.domain,
                            y_ticks,
                        );
                        let x_ticks = infer_x_ticks(
                            args.flag_x_ticks,
                            &mut formatter,
                            &x_axis_info,
                            &y_ticks_labels,
                            cols / actual_grid_cols,
                        );
                        let y_axis = Axis::default()
                            .title(if y_axis_info.can_be_displayed {
                                if has_added_series {
                                    single_finalized_series.0.unwrap().dim()
                                } else {
                                    y_column_name.clone().dim()
                                }
                            } else {
                                "".dim()
                            })
                            .style(Style::default().white())
                            .bounds([
                                y_axis_info.domain.0.as_float(),
                                y_axis_info.domain.1.as_float(),
                            ])
                            .labels(y_ticks_labels);

                        // Create the X axis and define its properties
                        let x_ticks_labels = graduations_from_domain(
                            &mut formatter,
                            x_axis_info.axis_type,
                            x_axis_info.domain,
                            x_ticks,
                        );
                        let x_axis = Axis::default()
                            .title(if x_axis_info.can_be_displayed {
                                x_column_name.clone().dim()
                            } else {
                                "".dim()
                            })
                            .style(Style::default().white())
                            .bounds([
                                x_axis_info.domain.0.as_float(),
                                x_axis_info.domain.1.as_float(),
                            ])
                            .labels(x_ticks_labels.clone());

                        // Create the chart and link all the parts together
                        let mut chart = Chart::new(vec![dataset]).x_axis(x_axis).y_axis(y_axis);

                        if category_column_index.is_some() {
                            chart = chart.hidden_legend_constraints((
                                Constraint::Min(0),
                                Constraint::Min(0),
                            ));
                        } else {
                            chart = chart.legend_position(None);
                        }

                        frame.render_widget(chart, layout[i]);
                        patch_buffer(
                            frame.buffer_mut(),
                            Some(&layout[i]),
                            &x_ticks_labels,
                            args.flag_grid,
                        );

                        color_i += 1;
                    }
                })?;

                print_terminal(&terminal, cols);
            }
        }
    }

    Ok(())
}

fn parse_as_timestamp(cell: &[u8]) -> Result<DynamicNumber, CliError> {
    let format_error = || {
        CliError::Other(format!(
            "could not parse \"{}\" as date!",
            String::from_utf8_lossy(cell)
        ))
    };

    let string = std::str::from_utf8(cell).map_err(|_| format_error())?;

    let zoned = if let Ok(z) = string.parse::<Zoned>() {
        z
    } else if let Ok(datetime) = string.parse::<DateTime>() {
        datetime
            .to_zoned(TimeZone::system())
            .map_err(|_| format_error())?
    } else if let Ok(date) = string.parse::<Date>() {
        date.to_datetime(Time::default())
            .to_zoned(TimeZone::system())
            .map_err(|_| format_error())?
    } else {
        return Err(format_error());
    };

    Ok(DynamicNumber::Integer(zoned.timestamp().as_millisecond()))
}

fn parse_as_number(cell: &[u8]) -> Result<DynamicNumber, CliError> {
    let string = String::from_utf8_lossy(cell);

    string
        .parse::<DynamicNumber>()
        .map_err(|_| CliError::Other(format!("could not parse \"{}\" as number!", string)))
}

impl DynamicNumber {
    fn to_timestamp(self) -> Timestamp {
        Timestamp::from_millisecond(self.as_int()).unwrap()
    }

    fn to_zoned(self) -> Zoned {
        self.to_timestamp().to_zoned(TimeZone::UTC)
    }
}

const MEAN_COLS: i64 = 35;
const MINUTES_BOUND: i64 = 60;
const HOURS_BOUND: i64 = MINUTES_BOUND * 60;
const DAYS_BOUND: i64 = HOURS_BOUND * 24;
const MONTHS_BOUND: i64 = DAYS_BOUND * 60;
const YEARS_BOUND: i64 = MONTHS_BOUND * 12;

fn infer_temporal_granularity(domain: (DynamicNumber, DynamicNumber)) -> Unit {
    let start = domain.0.to_zoned();
    let end = domain.1.to_zoned();

    let duration = start.duration_until(&end);
    let seconds = duration.as_secs();

    if seconds > YEARS_BOUND * MEAN_COLS {
        Unit::Year
    } else if seconds > MONTHS_BOUND * MEAN_COLS {
        Unit::Month
    } else if seconds > DAYS_BOUND * MEAN_COLS {
        Unit::Day
    } else if seconds > HOURS_BOUND * MEAN_COLS {
        Unit::Hour
    } else if seconds > MINUTES_BOUND * MEAN_COLS {
        Unit::Minute
    } else {
        Unit::Second
    }
}

fn floor_timestamp(milliseconds: DynamicNumber, unit: Unit) -> i64 {
    let mut zoned = milliseconds.to_zoned();

    // TODO: we could optimize some computations by foregoing
    zoned = match unit {
        Unit::Year => zoned.start_of_day().unwrap().first_of_year().unwrap(),
        Unit::Month => zoned.start_of_day().unwrap().first_of_month().unwrap(),
        _ => zoned.round(ZonedRound::new().smallest(unit)).unwrap(),
    };

    zoned.timestamp().as_millisecond()
}

fn format_timestamp(milliseconds: i64, unit: Unit) -> String {
    let timestamp = Timestamp::from_millisecond(milliseconds).unwrap();

    timestamp
        .strftime(match unit {
            Unit::Year => "%Y",
            Unit::Month => "%Y-%m",
            Unit::Day => "%F",
            _ => "%F %T",
        })
        .to_string()
}

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
    if let AxisType::Timestamp(granularity) = axis_type {
        return format_timestamp(x.trunc() as i64, granularity);
    }

    util::pretty_print_float(
        formatter,
        match axis_type {
            AxisType::Float => x,
            AxisType::Int => x.trunc(),
            _ => unreachable!(),
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

    if axis_type.is_int() {
        let range = (domain.1.as_int() - domain.0.as_int()).abs();

        if steps as i64 >= range {
            return (domain.0.as_int()..domain.1.as_int())
                .map(|i| i.to_string())
                .collect();
        }
    }

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

fn fix_flat_domain(
    domain: (DynamicNumber, DynamicNumber),
    axis_type: AxisType,
) -> (DynamicNumber, DynamicNumber) {
    if domain.0 != domain.1 {
        return domain;
    }

    match axis_type {
        AxisType::Float => {
            let center_value = domain.0.as_float();
            (
                DynamicNumber::Float(center_value - 0.5),
                DynamicNumber::Float(center_value + 0.5),
            )
        }
        AxisType::Int => {
            let center_value = domain.0.as_int();
            (
                DynamicNumber::Integer(center_value - 1),
                DynamicNumber::Integer(center_value + 1),
            )
        }
        _ => unimplemented!(),
    }
}

#[derive(Clone)]
struct AxisInfo {
    can_be_displayed: bool,
    domain: (DynamicNumber, DynamicNumber),
    axis_type: AxisType,
}

impl AxisInfo {
    fn from_single_series(series: &(Option<String>, Series)) -> (AxisInfo, AxisInfo) {
        Self::from_multiple_series(std::iter::once(series))
    }

    fn from_multiple_series<'a>(
        mut series: impl Iterator<Item = &'a (Option<String>, Series)>,
    ) -> (AxisInfo, AxisInfo) {
        let first_series = &series.next().unwrap().1;
        let mut x_domain = first_series.x_domain().unwrap();
        let mut y_domain = first_series.y_domain().unwrap();
        let mut x_axis_type = first_series.types.0;
        let mut y_axis_type = first_series.types.1;
        let mut x_can_be_displayed = true;
        let mut y_can_be_displayed = true;

        for (_, other_series) in series {
            let other_x_domain = other_series.x_domain().unwrap();
            let other_y_domain = other_series.y_domain().unwrap();

            if other_x_domain.0 < x_domain.0 {
                x_domain.0 = other_x_domain.0;
            }
            if other_x_domain.1 > x_domain.1 {
                x_domain.1 = other_x_domain.1;
            }

            if other_y_domain.0 < y_domain.0 {
                y_domain.0 = other_y_domain.0;
            }
            if other_y_domain.1 > y_domain.1 {
                y_domain.1 = other_y_domain.1;
            }

            x_axis_type = x_axis_type.and(other_series.types.0);
            y_axis_type = y_axis_type.and(other_series.types.1);

            if !other_series.can_display_x_axis_title() {
                x_can_be_displayed = false;
            }
            if !other_series.can_display_y_axis_title() {
                y_can_be_displayed = false;
            }
        }

        x_domain = fix_flat_domain(x_domain, x_axis_type);
        y_domain = fix_flat_domain(y_domain, y_axis_type);

        (
            AxisInfo {
                can_be_displayed: x_can_be_displayed,
                domain: x_domain,
                axis_type: x_axis_type,
            },
            AxisInfo {
                can_be_displayed: y_can_be_displayed,
                domain: y_domain,
                axis_type: y_axis_type,
            },
        )
    }
}

fn print_terminal(terminal: &Terminal<TestBackend>, cols: usize) {
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
            _ => unimplemented!(),
        };

        if modifer.is_empty() {
            return string;
        }

        match modifer {
            Modifier::DIM => Colorize::dimmed(string),
            _ => unimplemented!(),
        }
    }

    while i < contents.len() {
        let line = group_cells_by_color(&contents[i..(i + cols)])
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

        i += cols;
    }
}

fn patch_buffer(buffer: &mut Buffer, area: Option<&Rect>, x_ticks: &[String], draw_grid: bool) {
    let area = *area.unwrap_or(buffer.area());

    let origin_col = (area.x..area.x + area.width)
        .find(|x| buffer.cell((*x, area.y)).unwrap().symbol() == "│")
        .unwrap();

    // Drawing ticks for y axis
    for y in area.y..area.y + area.height {
        let cell = buffer.cell((origin_col, y)).unwrap();

        if cell.symbol() == "│"
            && (area.x..origin_col).any(|x| buffer.cell((x, y)).unwrap().symbol() != " ")
        {
            buffer.cell_mut((origin_col, y)).unwrap().set_symbol("┼");

            if y > area.y && y < area.y + area.height - 3 && draw_grid {
                for x in origin_col + 1..area.x + area.width {
                    let cell = buffer.cell_mut((x, y)).unwrap();

                    if cell.symbol() == " " {
                        cell.reset();
                        cell.set_symbol("─");
                        cell.set_style(Style::new().dim());
                    }
                }
            }
        }
    }

    // Fixing ticks for x axis
    let x_axis_line_y = area.y + area.height - 2;
    let x_axis_legend_y = x_axis_line_y + 1;
    let x_axis_end_pos = (area.x + area.width - 1, x_axis_line_y);

    for x in area.x..area.x + area.width {
        buffer.cell_mut((x, x_axis_legend_y)).unwrap().reset();
    }

    let steps = x_ticks.len();
    let mut t = 0.0;
    let fract = 1.0 / (steps - 1) as f64;

    let first_tick = x_ticks.first().unwrap();
    buffer.set_string(
        origin_col
            .saturating_sub(first_tick.width() as u16)
            .min(area.x),
        x_axis_legend_y,
        first_tick,
        Style::new(),
    );

    for tick in x_ticks.iter().skip(1).take(steps - 2) {
        t += fract;
        let x = lerp(origin_col as f64, (area.x + area.width - 1) as f64, t) as u16;
        buffer.cell_mut((x, x_axis_line_y)).unwrap().set_symbol("┼");

        buffer.set_string(
            x - (tick.width() / 2) as u16,
            x_axis_legend_y,
            tick,
            Style::new(),
        );

        if draw_grid {
            for y in area.y..x_axis_line_y {
                let cell = buffer.cell_mut((x, y)).unwrap();

                if cell.symbol() == " " {
                    cell.reset();
                    cell.set_symbol("│");
                    cell.set_style(Style::new().dim());
                }

                if cell.symbol() == "─" {
                    cell.set_symbol("┼");
                    cell.set_style(Style::new().dim());
                }
            }
        }
    }

    let last_tick = x_ticks.last().unwrap();

    buffer.set_string(
        area.x + area.width - last_tick.width() as u16,
        x_axis_legend_y,
        last_tick,
        Style::new(),
    );
    buffer.cell_mut(x_axis_end_pos).unwrap().set_symbol("┼");
}

fn infer_x_ticks(
    from_user: Option<NonZeroUsize>,
    formatter: &mut numfmt::Formatter,
    x_axis_info: &AxisInfo,
    y_ticks_labels: &[String],
    mut cols: usize,
) -> usize {
    if let Some(n) = from_user {
        return n.get().max(2);
    }

    let sample = graduations_from_domain(formatter, x_axis_info.axis_type, x_axis_info.domain, 15);

    let y_offset = y_ticks_labels.first().unwrap().width() + 1;
    cols = cols.saturating_sub(y_offset);

    let max_width = sample.iter().map(|label| label.width()).max().unwrap() + 4;

    (cols / max_width).max(2)
}

#[derive(Debug, Clone, Copy)]
enum AxisType {
    Int,
    Float,
    Timestamp(Unit),
}

impl AxisType {
    fn and(self, other: AxisType) -> Self {
        match (self, other) {
            (Self::Timestamp(unit1), Self::Timestamp(unit2)) => Self::Timestamp(unit1.max(unit2)),
            (Self::Float, Self::Int) | (Self::Int, Self::Float) | (Self::Float, Self::Float) => {
                Self::Float
            }
            (Self::Int, Self::Int) => Self::Int,
            _ => unreachable!(),
        }
    }

    fn is_int(self) -> bool {
        matches!(self, AxisType::Int)
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

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn to_floats(&self) -> Vec<(f64, f64)> {
        self.points
            .iter()
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

    fn mark_as_temporal(&mut self, granularity: Option<Unit>) {
        if let Some((x_domain, y_domain)) = self.extent.as_mut() {
            let granularity = granularity.unwrap_or_else(|| infer_temporal_granularity(*x_domain));
            self.types.0 = AxisType::Timestamp(granularity);

            let mut buckets: HashMap<i64, DynamicNumber> = HashMap::new();

            for (x, y) in self.points.iter() {
                buckets
                    .entry(floor_timestamp(*x, granularity))
                    .and_modify(|c| *c += *y)
                    .or_insert(*y);
            }

            self.points.clear();

            let mut new_y_domain: Option<(DynamicNumber, DynamicNumber)> = None;

            for (x, y) in buckets {
                match new_y_domain.as_mut() {
                    None => new_y_domain = Some((y, y)),
                    Some((current_y_min, current_y_max)) => {
                        if y < *current_y_min {
                            *current_y_min = y;
                        }

                        if y > *current_y_max {
                            *current_y_max = y;
                        }
                    }
                };

                self.points.push((DynamicNumber::Integer(x), y));
            }

            *x_domain = (
                DynamicNumber::Integer(floor_timestamp(x_domain.0, granularity)),
                DynamicNumber::Integer(floor_timestamp(x_domain.1, granularity)),
            );
            *y_domain = new_y_domain.unwrap();
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

#[derive(Default)]
struct CategoricalSeries {
    mapping: IndexMap<Vec<u8>, Series>,
}

impl CategoricalSeries {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, name: Vec<u8>, x: DynamicNumber, y: DynamicNumber) {
        self.mapping
            .entry(name)
            .and_modify(|series| {
                series.add(x, y);
            })
            .or_insert_with(|| Series::of(x, y));
    }

    fn into_finalized_series(self) -> Vec<(Option<String>, Series)> {
        self.mapping
            .into_iter()
            .map(|(name, series)| (Some(String::from_utf8(name).unwrap()), series))
            .collect()
    }

    fn is_empty(&self) -> bool {
        self.mapping.values().all(|series| series.is_empty())
    }
}

#[derive(Default)]
struct MultipleSeries {
    series: Vec<(String, Series)>,
}

impl MultipleSeries {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            series: Vec::with_capacity(capacity),
        }
    }

    fn register_series(&mut self, name: &[u8]) {
        self.series
            .push((String::from_utf8(name.to_vec()).unwrap(), Series::new()));
    }

    fn add(&mut self, index: usize, x: DynamicNumber, y: DynamicNumber) {
        self.series[index].1.add(x, y);
    }

    fn into_finalized_series(self) -> Vec<(Option<String>, Series)> {
        self.series
            .into_iter()
            .map(|(name, series)| (Some(name), series))
            .collect()
    }

    fn is_empty(&self) -> bool {
        self.series.iter().all(|(_, series)| series.is_empty())
    }
}

enum SeriesBuilder {
    Single(Series),
    Multiple(MultipleSeries),
    Categorical(CategoricalSeries),
}

impl SeriesBuilder {
    fn new_single() -> Self {
        Self::Single(Series::new())
    }

    fn new_categorical() -> Self {
        Self::Categorical(CategoricalSeries::new())
    }

    fn add_with_name(&mut self, name: Vec<u8>, x: DynamicNumber, y: DynamicNumber) {
        match self {
            Self::Categorical(inner) => inner.add(name, x, y),
            _ => unreachable!(),
        };
    }

    fn add_with_index(&mut self, index: usize, x: DynamicNumber, y: DynamicNumber) {
        match self {
            Self::Multiple(inner) => inner.add(index, x, y),
            _ => unreachable!(),
        };
    }

    fn add(&mut self, x: DynamicNumber, y: DynamicNumber) {
        match self {
            Self::Single(inner) => inner.add(x, y),
            _ => unreachable!(),
        };
    }

    fn into_finalized_series(self) -> Vec<(Option<String>, Series)> {
        match self {
            Self::Single(inner) => vec![(None, inner)],
            Self::Categorical(inner) => inner.into_finalized_series(),
            Self::Multiple(inner) => inner.into_finalized_series(),
        }
    }

    fn is_empty(&self) -> bool {
        match self {
            Self::Single(inner) => inner.is_empty(),
            Self::Multiple(inner) => inner.is_empty(),
            Self::Categorical(inner) => inner.is_empty(),
        }
    }
}
