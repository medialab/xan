use std::io::ErrorKind::BrokenPipe;
use std::time::Duration;

use bytesize::MB;
use csv;
use indicatif::{HumanCount, ProgressBar, ProgressStyle};

use config::{Config, Delimiter};
use util;
use CliResult;

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

fn get_progress_style(total: &Option<u64>, color: &str) -> ProgressStyle {
    ProgressStyle::with_template(&match total {
        Some(count) => get_progress_style_template(*count, color),
        None => "{prefix} {human_pos} rows {spinner} in {elapsed} ({per_sec})".to_string(),
    })
    .unwrap()
    .progress_chars("━╸━")
    .tick_chars("⠁⠁⠉⠙⠚⠒⠂⠂⠒⠲⠴⠤⠄⠄⠤⠠⠠⠤⠦⠖⠒⠐⠐⠒⠓⠋⠉⠈⠈⣿")
}

#[derive(Debug, Clone)]
struct EnhancedProgressBar {
    inner: ProgressBar,
}

impl EnhancedProgressBar {
    fn new(total: Option<u64>, title: Option<String>) -> Self {
        let bar = match total {
            None => ProgressBar::new_spinner(),
            Some(count) => ProgressBar::new(count),
        };

        bar.set_style(get_progress_style(&total, "blue"));

        if let Some(string) = title {
            bar.set_prefix(string);
        }

        bar.enable_steady_tick(Duration::from_millis(100));

        Self { inner: bar }
    }

    fn inc(&self, delta: u64) {
        self.inner.inc(delta);
    }

    fn change_color(&self, color: &str) {
        self.inner
            .set_style(get_progress_style(&self.inner.length(), color));
    }

    fn interrupt(&self) {
        eprint!("\x1b[1A");
        self.change_color("yellow");
        self.inner.abandon();
    }

    fn fail(&self) {
        self.change_color("red");
        self.inner.abandon();
    }

    fn succeed(&self) {
        self.change_color("green");
        self.inner.abandon();
    }
}

static USAGE: &str = "
Display a progress bar while reading the rows of a CSV file.

The command will try and buffer some of the ingested file to find
the total number of rows automatically. If you know the total
beforehand, you can also use the --total flag.

Usage:
    xan progress [options] [<input>]
    xan progress --help

progress options:
    -S, --smooth         Flush output buffer each time one row is written.
                         This makes the progress bar smoother, but might be
                         less performant.
    -B, --prebuffer <n>  Number of megabytes of the file to prebuffer to attempt
                         knowing the progress bar total automatically.
                         [default: 64]
    --title <string>     Title of the loading bar.
    --total <n>          Total number of rows of given CSV file.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will be included in
                           the progress bar total.
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_title: Option<String>,
    flag_prebuffer: u64,
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

    console::set_colors_enabled(true);

    let mut rdr = conf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    conf.write_headers(&mut rdr, &mut wtr)?;

    let mut record = csv::ByteRecord::new();
    let mut total = args.flag_total;

    let mut buffer: Vec<csv::ByteRecord> = Vec::new();

    if total.is_none() {
        let upper_bound = args.flag_prebuffer * MB;
        let mut read_all = true;

        while rdr.read_byte_record(&mut record)? {
            buffer.push(record.clone());

            if record.position().unwrap().byte() >= upper_bound {
                read_all = false;
                break;
            }
        }

        if read_all {
            total = Some(buffer.len() as u64);
        }
    }

    let bar = EnhancedProgressBar::new(total, args.flag_title);

    // NOTE: dealing with voluntary interruptions
    let bar_handle = bar.clone();

    ctrlc::set_handler(move || {
        bar_handle.interrupt();
        std::process::exit(1);
    })
    .unwrap();

    macro_rules! handle_row {
        ($record:ident) => {
            wtr.write_byte_record(&$record)
                .map_err(|err| match err.kind() {
                    csv::ErrorKind::Io(inner_err) if inner_err.kind() == BrokenPipe => {
                        bar.fail();

                        err
                    }

                    _ => err,
                })?;

            if args.flag_smooth {
                wtr.flush().map_err(|err| {
                    if err.kind() == BrokenPipe {
                        bar.fail();
                    }

                    err
                })?;
            }

            bar.inc(1);
        };
    }

    for buffered_record in buffer {
        handle_row!(buffered_record);
    }

    while rdr.read_byte_record(&mut record)? {
        handle_row!(record);
    }

    bar.succeed();

    Ok(wtr.flush()?)
}
