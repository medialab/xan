use std::io;

use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;
use ratatui::style::{Style, Stylize};
use ratatui::symbols;
use ratatui::widgets::{Axis, Block, Chart, Dataset, GraphType};
use ratatui::Terminal;

use config::{Config, Delimiter};
use util;
use CliResult;

static USAGE: &str = "
TODO...

Usage:
    xsv scatter [options] [<input>]
    xsv scatter --help

scatte options:
    -x <column>  TODO...


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
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_x: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let size = util::acquire_term_rows().unwrap() as u16;

    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    terminal.clear()?;

    terminal.draw(|frame| {
        let area = frame.size();
        let area = Rect::new(0, 0, area.width, size.saturating_sub(1));
        // Create the datasets to fill the chart with
        let datasets = vec![
            // Scatter chart
            Dataset::default()
                .name("data1")
                .marker(symbols::Marker::Dot)
                .graph_type(GraphType::Scatter)
                .style(Style::default().cyan())
                .data(&[(0.0, 5.0), (1.0, 6.0), (1.5, 6.434)]),
            // Line chart
            Dataset::default()
                .name("data2")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().magenta())
                .data(&[(4.0, 5.0), (5.0, 8.0), (7.66, 13.5)]),
        ];

        // Create the X axis and define its properties
        let x_axis = Axis::default()
            .title("X Axis".red())
            .style(Style::default().white())
            .bounds([0.0, 10.0])
            .labels(vec!["0.0".into(), "5.0".into(), "10.0".into()]);

        // Create the Y axis and define its properties
        let y_axis = Axis::default()
            .title("Y Axis".red())
            .style(Style::default().white())
            .bounds([0.0, 10.0])
            .labels(vec!["0.0".into(), "5.0".into(), "10.0".into()]);

        // Create the chart and link all the parts together
        let chart = Chart::new(datasets)
            .block(Block::default().title("Chart"))
            .x_axis(x_axis)
            .y_axis(y_axis);

        frame.render_widget(chart, area);
    })?;

    // let mut rdr = rconf.reader()?;
    // let headers = rdr.byte_headers()?;

    // let mut record = csv::ByteRecord::new();

    // while rdr.read_byte_record(&mut record)? {
    //     dbg!(&record);
    // }

    Ok(())
}
