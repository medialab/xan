use std::time::Duration;

use console::set_colors_enabled;
use csv;
use indicatif::{HumanCount, ProgressBar, ProgressStyle};

use config::{Config, Delimiter};
use util;
use CliResult;

static USAGE: &str = "
Display a progress bar while reading the rows of a CSV file.

Usage:
    xan progress [options] [<input>]
    xan progress --help

progress options:
    -S, --smooth      Flush output buffer each time one row is written.
                      This makes the progress bar smoother, but might be
                      less performant.
    --title <string>  Title of the loading bar.
    --total <n>       Total number of rows of given CSV file.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will be included in
                           the progress bar total.
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

fn get_progress_style_template(total: u64, color: &str) -> String {
    let padding = HumanCount(total).to_string().len();

    let mut f = String::new();
    f.push_str("{prefix} {bar:40.");
    f.push_str(color);
    f.push_str("/white.dim} {human_pos:>");
    f.push_str(&padding.to_string());
    f.push_str("}/{human_len} rows {spinner} [{percent:>3}%] in {elapsed} ({per_sec}, eta: {eta})");

    f
}

fn get_progress_style(total: Option<u64>, color: &str) -> ProgressStyle {
    ProgressStyle::with_template(&match total {
        Some(count) => get_progress_style_template(count, color),
        None => "{prefix} {human_pos} rows {spinner} in {elapsed} ({per_sec})".to_string(),
    })
    .unwrap()
    .progress_chars("━╸━")
    .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⣿")
}

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_title: Option<String>,
    flag_total: Option<u64>,
    flag_smooth: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    set_colors_enabled(true);

    let mut rdr = conf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    conf.write_headers(&mut rdr, &mut wtr)?;

    let mut record = csv::ByteRecord::new();

    let bar = match args.flag_total {
        Some(total) => ProgressBar::new(total),
        None => ProgressBar::new_spinner(),
    };

    // NOTE: dealing with voluntary interruptions
    let bar_handle = bar.clone();
    let total_handle = args.flag_total.clone();

    ctrlc::set_handler(move || {
        eprint!("\x1b[1A");
        bar_handle.set_style(get_progress_style(total_handle, "yellow"));
        bar_handle.abandon();
    })
    .unwrap();

    bar.set_style(get_progress_style(args.flag_total, "blue"));
    bar.enable_steady_tick(Duration::from_millis(100));

    if let Some(title) = args.flag_title {
        bar.set_prefix(title);
    }

    while rdr.read_byte_record(&mut record)? {
        wtr.write_byte_record(&record)?;

        if args.flag_smooth {
            wtr.flush()?;
        }

        bar.inc(1);
    }

    bar.set_style(get_progress_style(args.flag_total, "green"));
    bar.abandon();

    Ok(wtr.flush()?)
}
