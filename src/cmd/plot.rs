// Issue tracking:
//  - https://github.com/ratatui/ratatui/issues/334
//  - https://github.com/ratatui/ratatui/issues/1391
use std::convert::TryFrom;
use std::io::{stderr, stdout, Write};
use std::num::NonZeroUsize;

use ahash::RandomState;
use indexmap::IndexMap;
use jiff::{
    civil::{Date, Time},
    tz::TimeZone,
    Timestamp, Unit, Zoned, ZonedRound,
};
use unicode_width::UnicodeWidthStr;

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::symbols;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType};

use crate::config::{Config, Delimiter};
use crate::dates::{infer_temporal_granularity, parse_partial_date, parse_zoned};
use crate::moonblade::GroupAggregationProgram;
use crate::ratatui::print_ratatui_frame_to_stdout;
use crate::scales::{Scale, ScaleType};
use crate::select::SelectedColumns;
use crate::util::{self, ColorMode};
use crate::{CliError, CliResult};

const TYPICAL_COLS: usize = 35;

#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
struct Marker(symbols::Marker);

impl Marker {
    fn into_inner(self) -> symbols::Marker {
        self.0
    }
}

impl TryFrom<String> for Marker {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self(match value.as_str() {
            "dot" => symbols::Marker::Dot,
            "braille" => symbols::Marker::Braille,
            "halfblock" => symbols::Marker::HalfBlock,
            "block" => symbols::Marker::Block,
            "bar" => symbols::Marker::Bar,
            _ => return Err(format!("unknown marker type \"{}\"!", value)),
        }))
    }
}

#[derive(Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
struct Granularity(Unit);

impl Granularity {
    fn into_inner(self) -> Unit {
        self.0
    }
}

impl TryFrom<String> for Granularity {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Self(match value.as_str() {
            "year" | "years" => Unit::Year,
            "month" | "months" => Unit::Month,
            "day" | "days" => Unit::Day,
            "hour" | "hours" => Unit::Hour,
            "minute" | "minutes" => Unit::Minute,
            "second" | "seconds" => Unit::Second,
            _ => return Err(format!("invalid granularity \"{}\"!", value)),
        }))
    }
}

static USAGE: &str = "
Draw a scatter plot or a line plot based on 2-dimensional data.

It is also possible to draw multiple series/lines, as well as drawing multiple
series/lines as small multiples (sometimes also called a facet grid), by providing
a -c/--category column or selecting multiple columns as <y> series.

Drawing a simple scatter plot:

    $ xan plot sepal_width sepal_length iris.csv

Drawing a categorical scatter plot:

    $ xan plot sepal_width sepal_length -c species iris.csv

The same, as small multiples:

    $ xan plot sepal_width sepal_length -c species iris.csv -S 2

As a line chart:

    $ xan plot -L sepal_length petal_length iris.csv

Plotting time series:

    $ xan plot -LT datetime units sales.csv

Plotting multiple comparable times series at once:

    $ xan plot -LT datetime amount,amount_fixed sales.csv

Different times series, as small multiples:

    $ xan plot -LT datetime revenue,units sales.csv -S 2

Usage:
    xan plot --count [options] <x> [<input>]
    xan plot [options] <x> <y> [<input>]
    xan plot --help

plot options:
    -L, --line                 Whether to draw a line plot instead of the default scatter plot.
    -B, --bars                 Whether to draw bars instead of the default scatter plot.
                               WARNING: currently does not work if y range does not include 0.
                               https://github.com/ratatui/ratatui/issues/1391
    -T, --time                 Use to indicate that the x axis is temporal. The axis will be
                               discretized according to some inferred temporal granularity and
                               y values will be summed wrt the newly discretized x axis.
    --count                    Omit the y column and count rows instead. Only relevant when
                               used with -T, --time that will discretize the x axis.
    -A, --aggregate <expr>     Expression that will be used to aggregate values falling into
                               the same bucket when discretizing the x axis, e.g. when using
                               the -T, --time flag. The `_` implicit variable will be use to
                               denote a value in said expression. For instance, if you want
                               to average the values you can pass `mean(_)`. Will default
                               to `sum(_)`.
    -c, --category <col>       Name of the categorical column that will be used to
                               draw distinct series per category.
                               Does not work when selecting multiple columns with <y>.
    -R, --regression-line      Draw a regression line. Only works when drawing a scatter plot with
                               a single series.
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
    -S, --small-multiples <n>  Display small multiples (also called facet grids) of datasets
                               given by -c, --category or when multiple series are provided to <y>,
                               using the provided number of grid columns. The plot will all share the same
                               x scale but use a different y scale by default. See --share-y-scale
                               and --separate-x-scale to tweak this behavior.
    --share-x-scale <yes|no>   Give \"yes\" to share x scale for all plot when drawing small multiples with -S,
                               or \"no\" to keep them separate.
                               [default: yes]
    --share-y-scale <yes|no>   Give \"yes\" to share y scale for all plot when drawing small multiples with -S,
                               or \"no\" to keep them separate. Defaults to \"yes\" when -c, --category is given
                               and \"no\" when multiple series are provided to <y>.
    -M, --marker <name>        Marker to use. Can be one of (by order of size): 'braille', 'dot',
                               'halfblock', 'bar', 'block'.
                               [default: braille]
    -G, --grid                 Draw a background grid.
    --x-ticks <n>              Approx. number of x-axis graduation steps. Will default to some
                               sensible number based on the dimensions of the terminal.
    --y-ticks <n>              Approx. number of y-axis graduation steps. Will default to some
                               sensible number based on the dimensions of the terminal.
    --x-min <n>                Force a minimum value for the x axis.
    --x-max <n>                Force a maximum value for the x axis.
    --y-min <n>                Force a minimum value for the y axis.
    --y-max <n>                Force a maximum value for the y axis.
    --x-scale <scale>          Apply a scale to the x axis. Can be one of \"lin\", \"log\",
                               \"log2\", \"log10\" or \"log(custom_base)\" like \"log(2.5)\".
                               [default: lin]
    --y-scale <scale>          Apply a scale to the y axis. Can be one of \"lin\", \"log\",
                               \"log2\", \"log10\" or \"log(custom_base)\" like \"log(2.5)\".
                               [default: lin]
    --color <when>             When to color the output using ANSI escape codes.
                               Use `auto` for automatic detection, `never` to
                               disable colors completely and `always` to force
                               colors, even when the output could not handle them.
                               [default: auto]
    -i, --ignore               Ignore values that cannot be correctly parsed.

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
    arg_x: SelectedColumns,
    arg_y: Option<SelectedColumns>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_line: bool,
    flag_bars: bool,
    flag_time: bool,
    flag_count: bool,
    flag_aggregate: Option<String>,
    flag_cols: Option<String>,
    flag_rows: Option<String>,
    flag_small_multiples: Option<NonZeroUsize>,
    flag_share_x_scale: String,
    flag_share_y_scale: Option<String>,
    flag_category: Option<SelectedColumns>,
    flag_regression_line: bool,
    flag_marker: Marker,
    flag_granularity: Option<Granularity>,
    flag_grid: bool,
    flag_x_ticks: Option<NonZeroUsize>,
    flag_y_ticks: Option<NonZeroUsize>,
    flag_x_min: Option<String>,
    flag_x_max: Option<String>,
    flag_y_min: Option<f64>,
    flag_y_max: Option<f64>,
    flag_x_scale: ScaleType,
    flag_y_scale: ScaleType,
    flag_color: ColorMode,
    flag_ignore: bool,
}

impl Args {
    fn parse_x_bounds(&self) -> CliResult<(Option<f64>, Option<f64>)> {
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
                    .map(|cell| parse_as_float(cell.as_bytes()))
                    .transpose()?,
                self.flag_x_max
                    .as_ref()
                    .map(|cell| parse_as_float(cell.as_bytes()))
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
    args.flag_color.apply();
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    if args.flag_time && !args.flag_x_scale.is_linear() {
        Err("--x-scale cannot be customized when using -T,--time")?;
    }

    debug_assert!(if args.flag_count {
        args.arg_y.is_none()
    } else {
        true
    });

    let (flag_x_min, flag_x_max) = args.parse_x_bounds()?;
    let (flag_y_min, flag_y_max) = (args.flag_y_min, args.flag_y_max);

    if args.flag_x_scale.is_logarithmic()
        && (matches!(flag_x_min, Some(v) if v <= 0.0) || matches!(flag_x_max, Some(v) if v <= 0.0))
    {
        Err("--x-min or --x-max cannot be <= 0 with --x-scale log!")?;
    }

    if args.flag_y_scale.is_logarithmic()
        && (matches!(flag_y_min, Some(v) if v <= 0.0) || matches!(flag_y_max, Some(v) if v <= 0.0))
    {
        Err("--y-min or --y-max cannot be <= 0 with --y-scale log!")?;
    }

    if matches!(args.flag_x_ticks, Some(n) if n.get() < 2) {
        Err("--x-ticks must be > 1!")?;
    }

    if matches!(args.flag_y_ticks, Some(n) if n.get() < 2) {
        Err("--y-ticks must be > 1!")?;
    }

    // Collecting data
    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?;

    let x_column_index = args.arg_x.single_selection(headers, !rconf.no_headers)?;

    let y_column_index_opt = args
        .arg_y
        .as_ref()
        .map(|name| name.selection(headers, !rconf.no_headers))
        .transpose()?
        .map(|s| s.into_first().unwrap());

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
        .map(|name| name.single_selection(headers, !rconf.no_headers))
        .transpose()?;

    let additional_series_indices = args
        .arg_y
        .as_ref()
        .map(|names| -> CliResult<Vec<(Vec<u8>, usize)>> {
            let sel = names.selection(headers, !rconf.no_headers)?.into_rest();

            let info: Vec<(Vec<u8>, usize)> = if args.flag_no_headers {
                sel.iter()
                    .map(|i| (i.to_string().into_bytes(), *i))
                    .collect()
            } else {
                sel.iter()
                    .zip(sel.select(headers))
                    .map(|(i, h)| (h.to_vec(), *i))
                    .collect()
            };

            Ok(info)
        })
        .transpose()?
        .unwrap_or_else(Vec::new);

    let has_added_series = !additional_series_indices.is_empty();

    if args.flag_category.is_some() && has_added_series {
        Err("-c, --category cannot work when multiple columns are given to <y>!")?;
    }

    if args.flag_regression_line {
        if args.flag_bars || args.flag_line {
            Err("-R/--regression-line does not work with -B/--bars nor -L/--line!")?;
        }

        if args.flag_category.is_some() || has_added_series {
            Err("-R/--regression-line only works with single series (e.g. when using -c/--category or when selecting multiple columns with <y>)!")?;
        }
    }

    let share_x_scale = args.flag_share_x_scale == "yes";
    let share_y_scale = args
        .flag_share_y_scale
        .as_ref()
        .map(|choice| choice == "yes")
        .unwrap_or(!has_added_series);

    let showing_multiple_series =
        category_column_index.is_some() || !additional_series_indices.is_empty();

    let mut record = simd_csv::ByteRecord::new();

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

    macro_rules! try_parse_as_float {
        ($scale: expr, $value: expr) => {{
            match parse_as_float($value) {
                Err(e) => {
                    if args.flag_ignore {
                        continue;
                    } else {
                        Err(e)?
                    }
                }
                Ok(v) => {
                    if $scale.is_logarithmic() && v <= 0.0 {
                        if args.flag_ignore {
                            continue;
                        } else {
                            Err("log scale encountered a value <= 0!")?
                        }
                    } else {
                        v
                    }
                }
            }
        }};
    }

    macro_rules! try_parse_as_timestamp {
        ($value: expr) => {{
            match parse_as_timestamp($value) {
                Err(e) => {
                    if args.flag_ignore {
                        continue;
                    } else {
                        Err(e)?
                    }
                }
                Ok(v) => v,
            }
        }};
    }

    while rdr.read_byte_record(&mut record)? {
        let x_cell = &record[x_column_index];

        let x = if args.flag_time {
            try_parse_as_timestamp!(x_cell)
        } else {
            try_parse_as_float!(args.flag_x_scale, x_cell)
        };

        let y = match y_column_index_opt {
            Some(y_column_index) => {
                try_parse_as_float!(args.flag_y_scale, &record[y_column_index])
            }
            None => 1.0,
        };

        // Filtering out-of-bounds values
        if matches!(flag_x_min, Some(x_min) if x < x_min)
            || matches!(flag_x_max, Some(x_max) if x > x_max)
            || matches!(flag_y_min, Some(y_min) if y < y_min)
            || matches!(flag_y_max, Some(y_max) if y > y_max)
        {
            continue;
        }

        if let Some(i) = category_column_index {
            series_builder.add_with_name(record[i].to_vec(), x, y)
        } else if !additional_series_indices.is_empty() {
            series_builder.add_with_index(0, x, y);

            for (i, (_, pos)) in additional_series_indices.iter().enumerate() {
                let v = try_parse_as_float!(args.flag_y_scale, &record[*pos]);

                if matches!(flag_y_min, Some(y_min) if v < y_min)
                    || matches!(flag_y_max, Some(y_max) if v > y_max)
                {
                    continue;
                }

                series_builder.add_with_index(i + 1, x, v);
            }
        } else {
            series_builder.add(x, y);
        }
    }

    if series_builder.is_empty() {
        writeln!(&mut stderr(), "Nothing to display!")?;
        return Ok(());
    }

    let mut finalized_series = series_builder.into_finalized_series();

    for (_, series) in finalized_series.iter_mut() {
        if args.flag_time {
            series.mark_as_temporal(
                args.flag_granularity.map(|g| g.into_inner()),
                args.flag_aggregate.as_deref().unwrap_or("sum(_)"),
            )?;
        }

        // Domain bounds
        if let Some(x_min) = flag_x_min {
            series.set_x_min(x_min);
        }
        if let Some(x_max) = flag_x_max {
            series.set_x_max(x_max);
        }
        if let Some(y_min) = flag_y_min {
            series.set_y_min(y_min);
        } else {
            // If y scale is not log and y domain is positive, we set min to 0
            if !args.flag_y_scale.is_logarithmic()
                && matches!(series.y_domain(), Some((y_min, _)) if y_min > 0.0)
            {
                series.set_y_min(0.0);
            }
        }
        if let Some(y_max) = flag_y_max {
            series.set_y_max(y_max);
        }

        // NOTE: we sort on x if we want a line plot
        if args.flag_line {
            series.sort_by_x_axis();
        }
    }

    // Solving cols & rows
    let cols = util::acquire_term_cols_ratio(&args.flag_cols)?;

    if cols < 10 {
        Err("not enough cols to draw!")?;
    }

    let mut rows = util::acquire_term_rows_ratio(&args.flag_rows)?;

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
    match args.flag_small_multiples {
        None => {
            print_ratatui_frame_to_stdout(cols, rows, |frame| {
                // let n = finalized_series[0].1.len();

                // x axis information
                let (x_axis_info, y_axis_info) = AxisInfo::from_multiple_series(
                    (args.flag_x_scale, args.flag_y_scale),
                    finalized_series.iter(),
                );

                let finalized_floats = finalized_series
                    .iter()
                    .map(|(name_opt, series)| {
                        (
                            name_opt,
                            series.to_scaled_floats((&x_axis_info.scale, &y_axis_info.scale)),
                            args.flag_regression_line.then(|| {
                                series.regression_line_endpoints((
                                    &x_axis_info.scale,
                                    &y_axis_info.scale,
                                ))
                            }),
                        )
                    })
                    .collect::<Vec<_>>();

                let datasets: Vec<_> = finalized_floats
                    .iter()
                    .enumerate()
                    .flat_map(|(i, (name_opt, data, reg_points))| {
                        let mut datasets = Vec::new();

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
                            .data(data);

                        if let Some(name) = name_opt {
                            dataset = dataset.name(name.clone());
                        }

                        datasets.push(dataset);

                        if let Some(Some(points)) = reg_points {
                            datasets.push(
                                Dataset::default()
                                    .marker(symbols::Marker::Braille)
                                    .graph_type(GraphType::Line)
                                    .red()
                                    .data(points),
                            )
                        }

                        datasets
                    })
                    .collect();

                // Create the Y axis and define its properties
                let y_ticks_labels = y_axis_info.ticks(y_ticks);
                let x_ticks = infer_x_ticks(args.flag_x_ticks, &x_axis_info, &y_ticks_labels, cols);

                let y_axis = Axis::default()
                    .title(if !has_added_series && y_axis_info.can_be_displayed {
                        y_column_name.dim()
                    } else {
                        "".dim()
                    })
                    .labels_alignment(Alignment::Right)
                    .style(Style::default().white())
                    .bounds([0.0, 1.0])
                    .labels(y_ticks_labels);

                // Create the X axis and define its properties
                let x_ticks_labels = x_axis_info.ticks(x_ticks);

                let x_axis = Axis::default()
                    .title(if x_axis_info.can_be_displayed {
                        x_column_name.dim()
                    } else {
                        "".dim()
                    })
                    .style(Style::default().white())
                    .bounds([0.0, 1.0])
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
        }
        Some(grid_cols) => {
            let grid_cols = grid_cols.get();
            let mut color_i: usize = 0;
            let mut first_grid_col = true;

            let (harmonized_x_axis_info, harmonized_y_axis_info) = AxisInfo::from_multiple_series(
                (args.flag_x_scale, args.flag_y_scale),
                finalized_series.iter(),
            );

            for finalized_series_column in finalized_series.chunks(grid_cols) {
                let x_column_name = x_column_name.clone();

                if first_grid_col {
                    first_grid_col = false;
                } else {
                    writeln!(&mut stdout())?;
                }

                print_ratatui_frame_to_stdout(cols, rows, |frame| {
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
                        let (mut x_axis_info, mut y_axis_info) = AxisInfo::from_single_series(
                            (args.flag_x_scale, args.flag_y_scale),
                            single_finalized_series,
                        );

                        if share_x_scale {
                            x_axis_info = harmonized_x_axis_info.clone();
                        }

                        if share_y_scale {
                            y_axis_info = harmonized_y_axis_info.clone();
                        }

                        // Create the datasets to fill the chart with
                        let can_display_y_axis_title =
                            single_finalized_series.1.can_display_y_axis_title();

                        let single_finalized_series = (
                            single_finalized_series.0.clone(),
                            single_finalized_series
                                .1
                                .to_scaled_floats((&x_axis_info.scale, &y_axis_info.scale)),
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

                        // Create the Y axis and define its properties
                        let y_ticks_labels = y_axis_info.ticks(y_ticks);
                        let x_ticks = infer_x_ticks(
                            args.flag_x_ticks,
                            &x_axis_info,
                            &y_ticks_labels,
                            cols / grid_cols,
                        );
                        let y_axis = Axis::default()
                            .title(if can_display_y_axis_title {
                                if has_added_series {
                                    single_finalized_series.0.unwrap().dim()
                                } else {
                                    y_column_name.clone().dim()
                                }
                            } else {
                                "".dim()
                            })
                            .labels_alignment(Alignment::Right)
                            .style(Style::default().white())
                            .bounds([0.0, 1.0])
                            .labels(y_ticks_labels);

                        // Create the X axis and define its properties
                        let x_ticks_labels = x_axis_info.ticks(x_ticks);

                        let x_axis = Axis::default()
                            .title(if x_axis_info.can_be_displayed {
                                x_column_name.clone().dim()
                            } else {
                                "".dim()
                            })
                            .style(Style::default().white())
                            .bounds([0.0, 1.0])
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
            }
        }
    }

    Ok(())
}

fn is_int(float: f64) -> bool {
    float.fract() <= f64::EPSILON
}

fn parse_as_timestamp(cell: &[u8]) -> Result<f64, CliError> {
    let format_error = || {
        CliError::Other(format!(
            "could not parse \"{}\" as date!",
            String::from_utf8_lossy(cell)
        ))
    };

    let string = std::str::from_utf8(cell).map_err(|_| format_error())?;

    let zoned = if let Ok(z) = parse_zoned(string, None, None) {
        z
    } else if let Ok(date) = string.parse::<Date>() {
        date.to_datetime(Time::default())
            .to_zoned(TimeZone::system())
            .map_err(|_| format_error())?
    } else if let Some(partial_date) = parse_partial_date(string) {
        partial_date
            .into_inner()
            .to_datetime(Time::default())
            .to_zoned(TimeZone::system())
            .map_err(|_| format_error())?
    } else {
        return Err(format_error());
    };

    Ok(zoned.timestamp().as_millisecond() as f64)
}

fn parse_as_float(cell: &[u8]) -> Result<f64, CliError> {
    fast_float::parse::<f64, &[u8]>(cell).map_err(|_| {
        CliError::Other(format!(
            "could not parse \"{}\" as number!",
            std::str::from_utf8(cell).unwrap_or("cannot decode")
        ))
    })
}

fn float_to_timestamp(float: f64) -> Timestamp {
    Timestamp::from_millisecond(float as i64).unwrap()
}

fn float_to_zoned(float: f64) -> Zoned {
    float_to_timestamp(float).to_zoned(TimeZone::system())
}

fn floor_timestamp(milliseconds: f64, unit: Unit) -> i64 {
    let mut zoned = float_to_zoned(milliseconds);

    // TODO: we could optimize some computations by foregoing
    zoned = match unit {
        Unit::Year => zoned.start_of_day().unwrap().first_of_year().unwrap(),
        Unit::Month => zoned.start_of_day().unwrap().first_of_month().unwrap(),
        _ => zoned.round(ZonedRound::new().smallest(unit)).unwrap(),
    };

    zoned.timestamp().as_millisecond()
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

fn fix_flat_domain(domain: (f64, f64), axis_type: AxisType) -> (f64, f64) {
    if domain.0 != domain.1 {
        return domain;
    }

    match axis_type {
        AxisType::Float => {
            let center_value = domain.0;
            (center_value - 0.5, center_value + 0.5)
        }
        AxisType::Int => {
            let center_value = domain.0;
            (center_value - 1.0, center_value + 1.0)
        }
        _ => domain, // We can do better but la flemme...
    }
}

#[derive(Clone, Debug)]
struct AxisInfo {
    can_be_displayed: bool,
    scale: Scale,
}

impl AxisInfo {
    fn from_single_series(
        scale_types: (ScaleType, ScaleType),
        series: &(Option<String>, Series),
    ) -> (AxisInfo, AxisInfo) {
        Self::from_multiple_series(scale_types, std::iter::once(series))
    }

    fn from_multiple_series<'a>(
        scale_types: (ScaleType, ScaleType),
        mut series: impl Iterator<Item = &'a (Option<String>, Series)>,
    ) -> (AxisInfo, AxisInfo) {
        let first_series = &series.next().unwrap().1;
        let mut x_domain = first_series.x_domain().unwrap();
        let mut y_domain = first_series.y_domain().unwrap();
        let mut x_axis_type = first_series.types.0;
        let mut y_axis_type = first_series.types.1;
        let mut x_can_be_displayed = first_series.can_display_x_axis_title();
        let mut y_can_be_displayed = first_series.can_display_y_axis_title();

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

        let x_scale = if let AxisType::Timestamp(unit) = x_axis_type {
            Scale::time(x_domain, (0.0, 1.0), unit)
        } else {
            Scale::nice(scale_types.0, x_domain, (0.0, 1.0), 10)
        };

        let y_scale = Scale::nice(scale_types.1, y_domain, (0.0, 1.0), 10);

        (
            AxisInfo {
                can_be_displayed: x_can_be_displayed,
                scale: x_scale,
            },
            AxisInfo {
                can_be_displayed: y_can_be_displayed,
                scale: y_scale,
            },
        )
    }

    fn ticks(&self, count: usize) -> Vec<String> {
        self.scale.formatted_ticks(count)
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
            .max(area.x),
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
    x_axis_info: &AxisInfo,
    y_ticks_labels: &[String],
    mut cols: usize,
) -> usize {
    if let Some(n) = from_user {
        return n.get().max(2);
    }

    let sample = x_axis_info.scale.formatted_ticks(15);

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
}

#[derive(Debug)]
struct Series {
    types: (AxisType, AxisType),
    points: Vec<(f64, f64)>,
    extent: Option<((f64, f64), (f64, f64))>,
}

impl Series {
    fn new() -> Self {
        Self {
            types: (AxisType::Int, AxisType::Int),
            points: Vec::new(),
            extent: None,
        }
    }

    fn to_scaled_floats(&self, scales: (&Scale, &Scale)) -> Vec<(f64, f64)> {
        self.points
            .iter()
            .map(|(x, y)| (scales.0.percent(*x), scales.1.percent(*y)))
            .collect()
    }

    fn len(&self) -> usize {
        self.points.len()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn of(x: f64, y: f64) -> Self {
        let mut series = Self::new();
        series.add(x, y);
        series
    }

    fn add(&mut self, x: f64, y: f64) {
        if !is_int(x) {
            self.types.0 = AxisType::Float;
        }

        if !is_int(y) {
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

    fn x_domain(&self) -> Option<(f64, f64)> {
        self.extent.map(|(x, _)| x)
    }

    fn y_domain(&self) -> Option<(f64, f64)> {
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

    fn mark_as_temporal(&mut self, granularity: Option<Unit>, expr: &str) -> CliResult<()> {
        if let Some((x_domain, y_domain)) = self.extent.as_mut() {
            let granularity = granularity.unwrap_or_else(|| {
                infer_temporal_granularity(
                    &float_to_zoned(x_domain.0),
                    &float_to_zoned(x_domain.1),
                    TYPICAL_COLS,
                )
            });
            self.types.0 = AxisType::Timestamp(granularity);

            let mut buckets = GroupAggregationProgram::<i64>::parse_without_headers(expr)?;

            if !buckets.has_single_expr() {
                Err(format!(
                    "-A, --aggregate should only have a single clause but found {} instead!",
                    buckets.len()
                ))?;
            }

            for (index, (x, y)) in self.points.iter().enumerate() {
                buckets.run_with(floor_timestamp(*x, granularity), index, *y)?;
            }

            self.points.clear();

            let mut new_y_domain: Option<(f64, f64)> = None;

            for result in buckets.iter() {
                let (x, y) = result?;
                let y = y.try_as_f64()?;

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

                self.points.push((x as f64, y));
            }

            *x_domain = (
                floor_timestamp(x_domain.0, granularity) as f64,
                floor_timestamp(x_domain.1, granularity) as f64,
            );
            *y_domain = new_y_domain.unwrap();
        }

        Ok(())
    }

    fn set_x_min(&mut self, v: f64) {
        if let Some((x, _)) = self.extent.as_mut() {
            x.0 = v;
        }
    }

    fn set_x_max(&mut self, v: f64) {
        if let Some((x, _)) = self.extent.as_mut() {
            x.1 = v;
        }
    }

    fn set_y_min(&mut self, v: f64) {
        if let Some((_, y)) = self.extent.as_mut() {
            y.0 = v;
        }
    }

    fn set_y_max(&mut self, v: f64) {
        if let Some((_, y)) = self.extent.as_mut() {
            y.1 = v;
        }
    }

    fn sort_by_x_axis(&mut self) {
        self.points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
    }

    fn regression_line(&self) -> Option<(f64, f64)> {
        if self.points.len() < 2 {
            return None;
        }

        let mut n = 0.0;
        let mut x = 0.0;
        let mut y = 0.0;
        let mut xy = 0.0;
        let mut xx = 0.0;

        for point in self.points.iter() {
            n += 1.0;
            x += (point.0 - x) / n;
            y += (point.1 - y) / n;
            xy += (point.0 * point.1 - xy) / n;
            xx += (point.0 * point.0 - xx) / n;
        }

        let delta = xx - x * x;
        let slope = if delta.abs() < 1e-24 {
            0.0
        } else {
            (xy - x * y) / delta
        };
        let intercept = y - slope * x;

        Some((intercept, slope))
    }

    fn regression_line_endpoints(&self, scales: (&Scale, &Scale)) -> Option<[(f64, f64); 2]> {
        self.regression_line().map(|(intercept, slope)| {
            let (min_x, max_x) = self.extent.unwrap().0;

            let mut first_point = (0.0, scales.1.percent(intercept + slope * min_x));
            let mut second_point = (1.0, scales.1.percent(intercept + slope * max_x));

            clip(&mut first_point, &mut second_point, [0.0, 0.0, 1.0, 1.0]);

            [first_point, second_point]
        })
    }
}

#[derive(Default)]
struct CategoricalSeries {
    mapping: IndexMap<Vec<u8>, Series, RandomState>,
}

impl CategoricalSeries {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, name: Vec<u8>, x: f64, y: f64) {
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

    fn add(&mut self, index: usize, x: f64, y: f64) {
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

    fn add_with_name(&mut self, name: Vec<u8>, x: f64, y: f64) {
        match self {
            Self::Categorical(inner) => inner.add(name, x, y),
            _ => unreachable!(),
        };
    }

    fn add_with_index(&mut self, index: usize, x: f64, y: f64) {
        match self {
            Self::Multiple(inner) => inner.add(index, x, y),
            _ => unreachable!(),
        };
    }

    fn add(&mut self, x: f64, y: f64) {
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

fn clip_t(n: f64, d: f64, c: &mut (f64, f64)) -> bool {
    let t_e = c.0;
    let t_l = c.1;

    if d.abs() < f64::EPSILON {
        return n < 0.0;
    }

    let t = n / d;

    if d > 0.0 {
        if t > t_l {
            return false;
        }
        if t > t_e {
            c.0 = t;
        }
    } else {
        if t < t_e {
            return false;
        }
        if t < t_l {
            c.1 = t;
        }
    }

    true
}

// NOTE: this is an implementation of Liang-Barsky clipping
// NOTE: true means inside
// NOTE: bb is [xmin, ymin, xmax, ymax]
fn clip(a: &mut (f64, f64), b: &mut (f64, f64), bb: [f64; 4]) -> bool {
    let x1 = a.0;
    let y1 = a.1;
    let x2 = b.0;
    let y2 = b.1;

    let dx = x2 - x1;
    let dy = y2 - y1;

    if dx.abs() < f64::EPSILON
        && dy.abs() < f64::EPSILON
        && x1 >= bb[0]
        && x1 <= bb[2]
        && y1 >= bb[1]
        && y1 <= bb[3]
    {
        return true;
    }

    let mut c = (0.0, 1.0);

    if clip_t(bb[0] - x1, dx, &mut c)
        && clip_t(x1 - bb[2], -dx, &mut c)
        && clip_t(bb[1] - y1, dy, &mut c)
        && clip_t(y1 - bb[3], -dy, &mut c)
    {
        let t_e = c.0;
        let t_l = c.1;

        if t_l < 1.0 {
            b.0 = x1 + t_l * dx;
            b.1 = y1 + t_l * dy;
        }
        if t_e > 0.0 {
            a.0 += t_e * dx;
            a.1 += t_e * dy;
        }

        return true;
    }

    false
}
