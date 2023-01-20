use std::fs;
use std::io;
use std::cmp;

use channel;
use csv;
use colored::Colorize;
use stats::{Frequencies, merge_all};
use threadpool::ThreadPool;
use unicode_width::UnicodeWidthStr;
use unicode_segmentation::UnicodeSegmentation;

use CliResult;
use config::{Config, Delimiter};
use index::Indexed;
use select::{SelectColumns, Selection};
use util;

static USAGE: &'static str = "
Compute a histogram on CSV data.

By default, there is a row for the N most frequent values for each field in the
data. The order and number of values can be tweaked with --asc and --limit,
respectively.

Since this computes an exact histogram, memory proportional to the
cardinality of each column is required.

Usage:
    xsv histogram [options] [<input>]

histogram options:
    -s, --select <arg>     Select a subset of columns to compute histograms
                           for. See 'xsv select --help' for the format
                           details. This is provided here because piping 'xsv
                           select' into 'xsv histogram' will disable the use
                           of indexing.
    -l, --limit <arg>      Limit the histogram to the N most common
                           items. Set to '0' to disable a limit.
                           [default: 10]
    -a, --asc              Sort the histogram in ascending order by
                           count. The default is descending order.
    --no-nulls             Don't include NULLs in the histogram.
    -j, --jobs <arg>       The number of jobs to run in parallel.
                           This works better when the given CSV data has
                           an index already created. Note that a file handle
                           is opened for each job.
                           When set to '0', the number of jobs is set to the
                           number of CPUs detected.
                           [default: 0]
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
                           in the histogram. Additionally, the 'field'
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
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_limit: usize,
    flag_asc: bool,
    flag_no_nulls: bool,
    flag_jobs: usize,
    flag_screen_size: usize,
    flag_bar_max: String,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let bar_max: &str = &args.flag_bar_max;
    if !BAR_MAX.contains(&bar_max) {
        return fail!(format!("Unknown \"{}\" bar-max found", bar_max));
    }

    let square_chars = vec!["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

    let mut screen_size = args.flag_screen_size;
    if screen_size == 0 {
        if let Some(size) = termsize::get() {
            screen_size = size.cols as usize;
        }
    }
    if screen_size < 80 {
        screen_size = 80;
    }

    let (headers, tables, lines_total) = match args.rconfig().indexed()? {
        Some(ref mut idx) if args.njobs() > 1 => args.parallel_ftables(idx),
        _ => args.sequential_ftables(),
    }?;

    let mut legend = "nb_lines | %     ".to_string();
    let lines_total_str = format_number(lines_total);
    let lines_total_str_len = UnicodeWidthStr::width(&lines_total_str[..]);
    if lines_total_str_len > 8 {
        legend = " ".repeat(lines_total_str_len - 8) + &legend;
    }
    let legend_str_len = UnicodeWidthStr::width(&legend[..]);

    if screen_size <= (legend_str_len + 2) {
        return fail!(format!("Too many lines in the input, we are not able to output the histogram."));
    }
    let size_bar_cols = (screen_size - (legend_str_len + 1)) / 3 * 2;
    let size_labels = screen_size - (legend_str_len + 1) - (size_bar_cols + 1);

    let head_ftables = headers.into_iter().zip(tables.into_iter());
    for (i, (header, ftab)) in head_ftables.enumerate() {
        let mut lines_done = 0;

        let mut header = String::from_utf8(header.to_vec()).unwrap().replace("\n", " ");
        header = header.replace("\r", " ");
        header = header.replace("\t", " ");
        header = header.replace("\u{200F}", "");
        header = header.replace("\u{200E}", "");
        if rconfig.no_headers {
            header = (i+1).to_string();
        }
        let mut header_str_len = UnicodeWidthStr::width(&header[..]);
        if header_str_len > size_labels {
            let moved_header = header.clone();
            let header_chars = UnicodeSegmentation::graphemes(&moved_header[..], true).collect::<Vec<&str>>();
            let mut it = cmp::min(size_labels - 1, header_chars.len());
            while header_str_len >= size_labels {
                header = header_chars[0..it].join("");
                header_str_len = UnicodeWidthStr::width(&header[..]);
                it -= 1;
            }
            header += "…";
            header_str_len += 1;
        }
        header = " ".repeat(size_labels - header_str_len) + &header + &" ".repeat(size_bar_cols);
        println!("{}\u{200E}  {}", header.yellow().bold(), legend.yellow().bold());

        let mut longest_bar = lines_total;

        let nb_categories_total = ftab.cardinality() as usize;
        
        let vec_ftables = args.counts(&ftab);
        if bar_max == "max" {
            longest_bar = 
                if vec_ftables.len() == 0
                    { 0 }
                else
                    {
                        if args.flag_asc
                            { vec_ftables[vec_ftables.len() - 1].1 as usize }
                        else
                            { vec_ftables[0].1 as usize }
                    };
        }

        for (j, (value, count)) in vec_ftables.into_iter().enumerate() {
            let count = count as f64;

            let mut value = String::from_utf8(value).unwrap().replace("\n", " ");
            value = value.replace("\r", " ");
            value = value.replace("\t", " ");
            value = value.replace("\u{200F}", "");
            value = value.replace("\u{200E}", "");
            let mut value_str_len = UnicodeWidthStr::width(&value[..]);
            if value_str_len > size_labels {
                let moved_value = value.clone();
                let value_chars = UnicodeSegmentation::graphemes(&moved_value[..], true).collect::<Vec<&str>>();
                let mut it = cmp::min(size_labels - 1, value_chars.len());
                while value_str_len >= size_labels {
                    value = value_chars[0..it].join("");
                    value_str_len = UnicodeWidthStr::width(&value[..]);
                    it -= 1;
                }
                value += "…";
                value_str_len += 1;
            }
            value = " ".repeat(size_labels - value_str_len) + &value.to_string();

            let mut count_int = count as usize;
            lines_done += count_int;
            let mut count_str = format_number(count_int);
            count_str = (" ".repeat(cmp::max(lines_total_str_len, 8) - count_str.chars().count())) + &count_str;

            count_int = count_int * size_bar_cols / longest_bar;
            let mut bar_str = square_chars[8].repeat(count_int);
            
            let count_float = count * size_bar_cols as f64 / longest_bar as f64;
            let remainder = ((count_float - count_int as f64) * 8.0) as usize;
            bar_str += square_chars[remainder % 8];

            let colored_bar_str =
                if j % 2 == 0
                    { bar_str.dimmed().white() }
                else
                    { bar_str.white() };

            if remainder % 8 != 0 {
                count_int += 1;
            }
            let empty = ".".repeat(size_bar_cols - count_int);

            println!(
                "{}\u{200E} {}{} {} | {}",
                value,
                &colored_bar_str,
                &empty,
                &count_str,
                &format!("{:.2}", (count * 100.0 / lines_total as f64))
            );
        }

        let nb_categories_done =
            if args.flag_limit != 0
                { cmp::min(args.flag_limit, nb_categories_total) }
            else
                { nb_categories_total };

        let resume =
            " ".repeat(size_labels + 1)
            + &"Histogram for ".to_owned()
            + &format_number(lines_done)
            + "/"
            + &lines_total_str
            + " lines and "
            + &format_number(nb_categories_done)
            + "/"
            + &format_number(nb_categories_total)
            + " categories.";
        println!("{}", resume.yellow().bold());
        println!("");
    }
    Ok(())
}

type ByteString = Vec<u8>;
type Headers = csv::ByteRecord;
type FTable = Frequencies<Vec<u8>>;
type FTables = Vec<Frequencies<Vec<u8>>>;

impl Args {
    fn rconfig(&self) -> Config {
        Config::new(&self.arg_input)
            .delimiter(self.flag_delimiter)
            .no_headers(self.flag_no_headers)
            .select(self.flag_select.clone())
    }

    fn counts(&self, ftab: &FTable) -> Vec<(ByteString, u64)> {
        let mut counts = if self.flag_asc {
            ftab.least_frequent()
        } else {
            ftab.most_frequent()
        };
        if self.flag_limit > 0 {
            counts = counts.into_iter().take(self.flag_limit).collect();
        }
        counts.into_iter().map(|(bs, c)| {
            if b"" == &**bs {
                (b"(NULL)"[..].to_vec(), c)
            } else {
                (bs.clone(), c)
            }
        }).collect()
    }

    fn sequential_ftables(&self) -> CliResult<(Headers, FTables, usize)> {
        let mut rdr = self.rconfig().reader()?;
        let (headers, sel) = self.sel_headers(&mut rdr)?;
        let (ftables, count) = self.ftables(&sel, rdr.byte_records())?;
        Ok((headers, ftables, count))
    }

    fn parallel_ftables(&self, idx: &mut Indexed<fs::File, fs::File>)
                       -> CliResult<(Headers, FTables, usize)> {
        let mut rdr = self.rconfig().reader()?;
        let (headers, sel) = self.sel_headers(&mut rdr)?;

        if idx.count() == 0 {
            return Ok((headers, vec![], 0));
        }

        let chunk_size = util::chunk_size(idx.count() as usize, self.njobs());
        let nchunks = util::num_of_chunks(idx.count() as usize, chunk_size);

        let pool = ThreadPool::new(self.njobs());
        let (send, recv) = channel::bounded(0);
        for i in 0..nchunks {
            let (send, args, sel) = (send.clone(), self.clone(), sel.clone());
            pool.execute(move || {
                let mut idx = args.rconfig().indexed().unwrap().unwrap();
                idx.seek((i * chunk_size) as u64).unwrap();
                let it = idx.byte_records().take(chunk_size);
                let (ftable, _) = args.ftables(&sel, it).unwrap();
                send.send(ftable);
            });
        }
        drop(send);
        Ok((headers, merge_all(recv).unwrap(), idx.count() as usize))
    }

    fn ftables<I>(&self, sel: &Selection, it: I) -> CliResult<(FTables, usize)>
            where I: Iterator<Item=csv::Result<csv::ByteRecord>> {
        let null = &b""[..].to_vec();
        let nsel = sel.normal();
        let mut tabs: Vec<_> =
            (0..nsel.len()).map(|_| Frequencies::new()).collect();
        let mut count = 0;
        for row in it {
            let row = row?;
            for (i, field) in nsel.select(row.into_iter()).enumerate() {
                if i == 0 {
                    count += 1;
                }
                let field = trim(field.to_vec());
                if !field.is_empty() {
                    tabs[i].add(field);
                } else {
                    if !self.flag_no_nulls {
                        tabs[i].add(null.clone());
                    }
                }
            }
        }
        Ok((tabs, count))
    }

    fn sel_headers<R: io::Read>(&self, rdr: &mut csv::Reader<R>)
                  -> CliResult<(csv::ByteRecord, Selection)> {
        let headers = rdr.byte_headers()?;
        let sel = self.rconfig().selection(headers)?;
        Ok((sel.select(headers).map(|h| h.to_vec()).collect(), sel))
    }

    fn njobs(&self) -> usize {
        if self.flag_jobs == 0 { util::num_cpus() } else { self.flag_jobs }
    }
}

fn trim(bs: ByteString) -> ByteString {
    match String::from_utf8(bs) {
        Ok(s) => s.trim().as_bytes().to_vec(),
        Err(bs) => bs.into_bytes(),
    }
}

fn format_number(count: usize) -> String {
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