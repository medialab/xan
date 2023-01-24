use std::cmp;

use csv;
use colored::Colorize;

use CliResult;
use config::{Config, Delimiter};
use select::{SelectColumns, Selection};
use util;

static USAGE: &'static str = "
Compute distribution on CSV data.

Usage:
    xsv dist [options] <column> [<input>]

dist options:
    -b, --bins <arg>       The number of bins in the distribution.
                           [default: 10]
    --min <arg>            The minimum from which we start to display
                           the distribution. When not set, will take the
                           minimum from the csv file.
                           [default: 0]
    --max <arg>            The maximum from which we start to display
                           the distribution. When not set, will take the
                           maximum from the csv file.
                           [default: 10]
    --no-nulls             Don't include NULLs in the dist table.
    --screen-size <arg>    The size used to output the histogram. Set to '0',
                           it will use the shell size.
                           [default: 0]
    --bar-max <arg>        The maximum value for a bar. If set to 'max', the maximum
                           possible size for a bar will be the maximum cardinality
                           of all bars in the histogram. If set to 'total', the maximum
                           possible size for a bar will be the sum of the cardinalities
                           of the bars.
                           [default: total]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 1-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. (default: ,)
";

static BAR_MAX: &'static [&'static str] = &[
    "max",
    "total",
];

#[derive(Clone, Deserialize)]
struct Args {
    arg_column: SelectColumns,
    arg_input: Option<String>,
    flag_bins: u64,
    flag_max: u64, // a passer en option
    flag_min: u64, // a passer en option
    flag_no_nulls: bool,
    flag_screen_size: usize,
    flag_bar_max: String,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = args.rconfig();

    let bar_max: &str = &args.flag_bar_max;
    if !BAR_MAX.contains(&bar_max) {
        return fail!(format!("Unknown \"{}\" bar-max found", bar_max));
    }

    let mut bar = Bar {
        screen_size: args.flag_screen_size,
        lines_total: 0,
        lines_total_str: String::new(),
        lines_total_str_len: 0,
        size_bar_cols: 0,
        size_labels: 0,
        longest_bar: 0,
    };

    let mut bins: Vec<(String, u64, u64, u64)> = match args.bins_construction() {
        Ok(bins) => {bins},
        Err(e) => return fail!(e),
    };

    let mut rdr = rconfig.reader()?;
    let headers = rdr.byte_headers()?;
    let sel = rconfig.selection(headers)?;

    let (bins_returned, lines_total) = match args.ftable(&sel, rdr.byte_records(), bins.as_mut_slice()) {
        Ok((bins_returned, lines_total)) => { (bins_returned, lines_total) },
        Err(e) => return fail!(e),
    };
    let bins_clone = bins_returned.clone();

    bar.lines_total = lines_total;
    match bar.update_sizes(args.flag_max) {
        Ok(_) => {},
        Err(e) => return fail!(e),
    };
    bar.print_title();

    bar.longest_bar = 0;
    if bar_max == "max" {
        for (_, _, _, count) in bins_returned.into_iter() {
            if count as usize > bar.longest_bar {
                bar.longest_bar = count as usize;
            }
        }
    } else {
        bar.longest_bar = lines_total as usize;
    }

    let mut lines_done = 0;
    for (j, res) in bins_clone.into_iter().enumerate() {
        lines_done += res.3;
        bar.print_bar(res.0, res.3 as u64, j);
    }

    let resume =
        " ".repeat(bar.size_labels + 1)
        + &"Distribution for ".to_owned()
        + &format_number(lines_done)
        + "/"
        + &bar.lines_total_str
        + " lines.";
    println!("{}", resume.yellow().bold());
    println!("");
    Ok(())
}

type ByteString = Vec<u8>;

impl Args {
    fn rconfig(&self) -> Config {
        Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_column.clone())
    }

    fn bins_construction(&self) -> CliResult<Vec<(String, u64, u64, u64)>> {
        let mut bins: Vec<(String, u64, u64, u64)> = Vec::new();
        let size_interval = (self.flag_max - self.flag_min) / self.flag_bins;
        let mut temp_min = self.flag_min;
        let mut temp_max = temp_min + size_interval;
        let mut temp_text = format_number(self.flag_min) + "-" + &format_number(temp_max);
        bins.push((temp_text, temp_min, temp_max, 0));
        for _ in 1..self.flag_bins {
            temp_min = temp_max;
            temp_max = temp_min + size_interval;
            temp_text = format_number(temp_min) + "-" + &format_number(temp_max);
            bins.push((temp_text, temp_min, temp_max, 0));
        }
        if !self.flag_no_nulls {
            bins.push(("(NULL)".to_string(), 0, 0, 0));
        }
        Ok(bins)
    }

    fn ftable<I>(&self, sel: &Selection, it: I, bins: &mut [(String, u64, u64, u64)]) -> CliResult<(Vec<(String, u64, u64, u64)>, u64)>
            where I: Iterator<Item=csv::Result<csv::ByteRecord>> {
        let bins_len = bins.len();
        let mut count = 0;
        for row in it {
            let row = row?;
            for field in sel.select(&row) {
                count += 1;
                let field = trim(field.to_vec());
                if !field.is_empty() {
                    let field_value = field.parse::<u64>().unwrap();
                    for (i, (_, min, max, _)) in bins.into_iter().enumerate() {
                        if *max > field_value && field_value >= *min {
                            bins[i].3 += 1;
                            break;
                        }
                    }
                } else {
                    if !self.flag_no_nulls {
                        bins[bins_len - 1].3 += 1;
                    }
                }
            }
        }
        Ok((bins.to_vec(), count))
    }
}

fn trim(bs: ByteString) -> String {
    match String::from_utf8(bs) {
        Ok(s) => s.trim().to_string(),
        Err(bs) => bs.to_string(),
    }
}

fn format_number(count: u64) -> String {
    let mut count_str = count.to_string();
    let count_len = count_str.chars().count();

    if count_len < 3 {
        return count_str;
    }

    let count_chars: Vec<char> = count_str.chars().collect();

    count_str = count_chars[0].to_string();
    for k in 1..count_len {
        if k % 3 == count_len % 3 {
            count_str += ",";
        }
        count_str += &count_chars[k].to_string();
    }
    return count_str;
}

struct Bar {
    screen_size: usize,
    lines_total: u64,
    lines_total_str: String,
    lines_total_str_len: usize,
    size_bar_cols: usize,
    size_labels: usize,
    longest_bar: usize,
}

impl Bar {

    fn update_sizes(&mut self, flag_max: u64) -> CliResult<()> {
        if self.screen_size == 0 {
            if let Some(size) = termsize::get() {
                self.screen_size = size.cols as usize;
            }
        }
        if self.screen_size < 80 {
            self.screen_size = 80;
        }

        self.lines_total_str = format_number(self.lines_total);
        self.lines_total_str_len = self.lines_total_str.chars().count();
        let mut legend_str_len = 17;
        if self.lines_total_str_len > 8 {
            legend_str_len = 17 + self.lines_total_str_len - 8;
        }

        if self.screen_size <= (legend_str_len + 2) {
            return fail!(format!("Too many lines in the input, we are not able to output the distribution."));
        }
        self.size_labels = cmp::max(cmp::max(self.lines_total_str_len, format_number(flag_max).chars().count()) * 2 + 1, 6);
        self.size_bar_cols = self.screen_size - (legend_str_len + 1) - (self.size_labels + 1);
        Ok(())
    }

    fn print_title(&mut self) {
        let mut legend = " ".repeat(self.size_labels + self.size_bar_cols) + &"nb_lines | %     ".to_string();
        if self.lines_total_str_len > 8 {
            legend = " ".repeat(self.lines_total_str_len - 8) + &legend;
        }
        println!("  {}", legend.yellow().bold());
    }

    fn print_bar(&mut self, value: String, count: u64, j: usize) {
        let square_chars = vec!["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

        let value = " ".repeat(self.size_labels - value.chars().count()) + &value.to_string();
        let count_str = format_number(count);

        let mut nb_square = count as usize * self.size_bar_cols / self.longest_bar;
        let mut bar_str = square_chars[8].repeat(nb_square);
        
        let count_float = count as f64 * self.size_bar_cols as f64 / self.longest_bar as f64;
        let remainder = ((count_float - nb_square as f64) * 8.0) as usize;
        bar_str += square_chars[remainder % 8];

        let colored_bar_str =
            if j % 2 == 0
                { bar_str.dimmed().white() }
            else
                { bar_str.white() };

        if remainder % 8 != 0 {
            nb_square += 1;
        }
        let empty = ".".repeat(self.size_bar_cols - nb_square as usize);

        let count_str = (" ".repeat(cmp::max(self.lines_total_str_len, 8) - count_str.chars().count())) + &count_str;

        println!(
            "{}\u{200E} {}{} {} | {}",
            value,
            &colored_bar_str,
            &empty,
            &count_str,
            &format!("{:.2}", (count as f64 * 100.0 / self.lines_total as f64))
        );
    }
}