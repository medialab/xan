use std::cmp;

use csv;
use colored::Colorize;

use CliResult;
use config::{Config, Delimiter};
use select::SelectColumns;
use util;

static USAGE: &'static str = "
Compute distribution on CSV data.

Usage:
    xsv dist [options] <column> [<input>]

dist options:
    --bins <arg>           The number of bins in the distribution.
                           [default: 10]
    --min <arg>            The minimum from which we start to display
                           the distribution. When not set, will take the
                           minimum from the csv file. If not set, will
                           search the min in the file.
    --max <arg>            The maximum from which we start to display
                           the distribution. When not set, will take the
                           maximum from the csv file. If not set, will
                           search the max in the file.
    --no-nans              Don't include NULLs and letters in the dist table.
    --screen-size <arg>    The size used to output the histogram. Set to '0',
                           it will use the shell size.
                           [default: 0]
    --bar-max <arg>        The maximum value for a bar. If set to 'max', the maximum
                           possible size for a bar will be the maximum cardinality
                           of all bars in the histogram. If set to 'total', the maximum
                           possible size for a bar will be the sum of the cardinalities
                           of the bars.
                           [default: total]
    --precision <arg>      The number of digit to keep after the comma. Has to be less
                           than 20.
                           [default: 2]

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
    flag_max: Option<f64>,
    flag_min: Option<f64>,
    flag_no_nans: bool,
    flag_screen_size: usize,
    flag_bar_max: String,
    flag_precision: u8,
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

    if let Some(min) = args.flag_min {
        if let Some(max) = args.flag_max {
            if max < min {
                return fail!("min must be less than max");
            }
        }
    }

    if args.flag_precision >= 20 {
        return fail!("precision must be less than 20");
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

    let mut min = match args.flag_min {
        None => f64::MAX,
        Some(min) => min,
    };
    let mut max = match args.flag_max {
        None => f64::MIN,
        Some(max) => max,
    };

    let mut values: Vec<f64> = Vec::new();
    let mut nans = 0;
    let mut lines_total = 0;

    let mut rdr = rconfig.reader()?;
    let headers = rdr.byte_headers()?;
    let sel = rconfig.selection(headers)?;
    let column_index = *sel.iter().next().unwrap();
    let mut record = csv::StringRecord::new();

    while rdr.read_record(&mut record)? {
        lines_total += 1;
        let cell = record[column_index].to_owned();
        let value = match cell.parse::<f64>() {
            Ok(nb) => nb,
            Err(_) => {
                if !args.flag_no_nans {
                    nans += 1;
                }
                continue
            }
        };
        values.push(value);
        if args.flag_min.is_none() && value < min as f64 {
            min = value;
        }
        if args.flag_max.is_none() && value > max as f64 {
            max = value;
        }
    }
    min = floor_float(min, args.flag_precision);
    max = ceil_float(max, args.flag_precision);
    if min > max {
        if min == floor_float(f64::MAX, args.flag_precision) || max == ceil_float(f64::MIN, args.flag_precision) {
            if nans == 0 {
                return fail!("No result because the colum is empty");
            } else {
                return fail!(format!("\"{}\" NaNs", nans));
            }
        }
        return fail!("No result because min is greater than max");
    }

    let max_nb_str_len = cmp::max(
        format_number_float(min, args.flag_precision).chars().count(),
        format_number_float(max, args.flag_precision).chars().count()
    );
    let max_label_len = cmp::max(max_nb_str_len * 2 + 3, 4);

    bar.lines_total = lines_total;
    match bar.update_sizes(max_label_len) {
        Ok(_) => {},
        Err(e) => return fail!(e),
    };
    bar.print_title();

    let (mut bins, size_interval) = match args.bins_construction(min, max, args.flag_precision) {
        Ok((bins, size_interval)) => { (bins, size_interval) },
        Err(e) => return fail!(e),
    };

    for value in values {
        if value > max || value < min{
            continue
        }
        let temp = (value - min) / size_interval;
        let mut pos = temp.floor() as usize;
        if pos as f64 == temp && pos != 0 {
            pos -= 1;
        }
        bins[pos].2 += 1;
    }

    bar.longest_bar = 0;
    if bar_max == "max" {
        for (_, _, count) in bins.clone().into_iter() {
            if count as usize > bar.longest_bar {
                bar.longest_bar = count as usize;
            }
        }
        if nans > bar.longest_bar {
            bar.longest_bar = nans;
        }
    } else {
        bar.longest_bar = lines_total as usize;
    }

    let mut lines_done = 0;
    let mut j = 0;
    for res in bins.into_iter() {
        lines_done += res.2;
        let min_interval = format_number_float(res.0, args.flag_precision);
        let max_interval = format_number_float(res.1, args.flag_precision);
        let interval = min_interval + " - " + &" ".repeat(max_nb_str_len - max_interval.chars().count()) + &max_interval;
        bar.print_bar(interval, res.2 as u64, j);
        j += 1;
    }
    if nans != 0 {
        lines_done += nans as u64;
        let interval = "NaNs";
        bar.print_bar(interval.to_string(), nans as u64, j);
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

impl Args {
    fn rconfig(&self) -> Config {
        Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.arg_column.clone())
    }

    fn bins_construction(&self, min: f64, max: f64, precision: u8) -> CliResult<(Vec<(f64, f64, u64)>, f64)> {
        let mut bins: Vec<(f64, f64, u64)> = Vec::new();
        let size_interval = ((max - min) / self.flag_bins as f64).abs();
        let mut temp_min = min;
        let mut temp_max = ceil_float(temp_min + size_interval, precision);
        bins.push((temp_min, temp_max, 0));
        for _ in 1..self.flag_bins {
            if size_interval == 0.0 {
                break;
            }
            temp_min = temp_max;
            temp_max = ceil_float(temp_min + size_interval, precision);
            bins.push((temp_min, temp_max, 0));
        }
        Ok((bins, size_interval))
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

fn format_number_float(count: f64, precision: u8) -> String {
    let neg = count < 0.0;
    let mut count_str = count.abs().to_string();
    let count_str_len = count_str.chars().count();
    let mut count_str_whole_len = count_str_len;
    if let Some(idx) = count_str.find(".") {
        count_str_whole_len = idx;
    }
    let count_chars: Vec<char> = count_str.chars().collect();
    count_str = count_chars[0].to_string();
    for k in 1..count_str_whole_len {
        if k % 3 == count_str_whole_len % 3 {
            count_str += ",";
        }
        count_str += &count_chars[k].to_string();
    }
    if count_str_whole_len == count_str_len {
        if precision != 0 {
            count_str += ".";
            count_str += &"0".repeat(precision as usize);
        }
    } else {
        for k in count_str_whole_len..count_str_len {
            count_str += &count_chars[k].to_string();
        }
        if (count_str_len -  count_str_whole_len) < (precision + 1) as usize {
            count_str += &"0".repeat((precision + 1) as usize - (count_str_len -  count_str_whole_len));   
        }
    }
    if neg {
        count_str = "-".to_string() + &count_str;
    }
    return count_str;
}

fn ceil_float(value: f64, precision: u8) -> f64 {
    let mul =
        if precision == 1 {
            1.0
        } else {
            u64::pow(10, precision as u32) as f64
        };
    return (value * mul).ceil() / mul;
}

fn floor_float(value: f64, precision: u8) -> f64 {
    let mul =
        if precision == 1 {
            1.0
        } else {
            u64::pow(10, precision as u32) as f64
        };
    return (value * mul).floor() / mul;
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

    fn update_sizes(&mut self, max_str_len: usize) -> CliResult<()> {
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
        self.size_labels = max_str_len + 1;
        if self.screen_size - (legend_str_len + 1) <= (self.size_labels + 1) {
            return fail!("Too precise to print the result, try lowering the precision");
        }
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
            if j % 2 == 0 {
                bar_str.dimmed().white()
            } else {
                bar_str.white()
            };

        if remainder % 8 != 0 {
            nb_square += 1;
        }
        let empty = ".".repeat(self.size_bar_cols - nb_square as usize);

        let count_str = (" ".repeat(cmp::max(self.lines_total_str_len, 8) - count_str.chars().count())) + &count_str;

        println!(
            "{} {}{} {} | {}",
            value,
            &colored_bar_str,
            &empty,
            &count_str,
            &format!("{:.2}", (count as f64 * 100.0 / self.lines_total as f64))
        );
    }
}