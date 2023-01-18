use std::fs;
use std::io;

use channel;
use csv;
use colored::Colorize;
use stats::{Frequencies, merge_all};
use threadpool::ThreadPool;

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
    -m, --max-size <arg>   The maximum size for a bar in the histogram.
                           Set to '0', will use the shell size to compute the
                           bar size.
                           [default: 0]
    --scale <arg>          The scale to choose. If set to max, the maximum possible
                           size for a bar will be the maximum cardinality of all bars.
                           If set to sum, the maximum possible size for a bar
                           will be the sum of the cardinalities of the bars.
                           [default: sum]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be included
                           in the histogram. Additionally, the 'field'
                           column will be 1-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. (default: ,)
";

static SCALE: &'static [&'static str] = &[
    "max",
    "sum",
];

#[derive(Clone, Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_limit: usize,
    flag_asc: bool,
    flag_no_nulls: bool,
    flag_jobs: usize,
    flag_max_size: usize,
    flag_scale: String,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);
    
    let mut max_size = args.flag_max_size;
    if max_size == 0 {
        let termsize::Size {rows: _, cols} = termsize::get().unwrap();
        max_size = cols as usize;
    }
    if max_size < 50 {
        max_size = 0;
    } else {
        max_size -= 50;
    }

    let scale: &str = &args.flag_scale;
    if !SCALE.contains(&scale) {
        return fail!(format!("Unknown \"{}\" scale found", scale));
    }
    let mut max_len = max_size;

    let square_chars = vec!["", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

    let (headers, tables, sum) = match args.rconfig().indexed()? {
        Some(ref mut idx) if args.njobs() > 1 => args.parallel_ftables(idx),
        _ => args.sequential_ftables(),
    }?;

    if scale == "sum" {
        max_len = sum;
    }

    let head_ftables = headers.into_iter().zip(tables.into_iter());
    for (i, (header, ftab)) in head_ftables.enumerate() {
        let mut header = String::from_utf8(header.to_vec()).unwrap();
        if rconfig.no_headers {
            header = (i+1).to_string();
        }
        if scale == "sum" {
            header += &(" ".repeat((32 + max_size) - header.chars().count()));
            header += &format_number(max_len);
        }
        println!("{}", header.yellow().bold());

        let mut count_str_max_size = 10;
        for (j, (value, count)) in args.counts(&ftab).into_iter().enumerate() {

            let mut value = String::from_utf8(value).unwrap();
            let value_len = value.chars().count();
            if value_len > 30 {
                let value_chars: Vec<char> = value.chars().collect();
                value = value_chars[0].to_string();
                for k in 1..29 {
                    value += &value_chars[k].to_string();
                }
                value += "…";
            }
            let nb_spaces = 31 - value.chars().count();
            value += &" ".repeat(nb_spaces);

            let mut count_int = count as usize;
            let count_str = format_number(count_int);
            if j == 0 {
                if scale == "max" {
                    max_len = count_int;
                }
                count_str_max_size = count_str.chars().count();
            }
            count_int = count_int * max_size / max_len;
            let mut bar = square_chars[8].repeat(count_int);

            let mut percentage_str = " ".repeat(count_str_max_size - count_str.chars().count()) + " | ";
            let count_float = count as f64 * max_size as f64 / max_len as f64;
            percentage_str += &format!("{:.2}", (count_float * 100.0 / max_size as f64));

            let remainder = ((count_float - count_int as f64) * 8.0) as usize;
            bar += square_chars[remainder % 8];

            if remainder % 8 != 0 {
                count_int += 1;
            }
            let empty = ".".repeat(max_size - count_int) + " ";

            let mut colored_bar = bar.white();
            if j % 2 == 1 {
                colored_bar = bar.dimmed().white();
            }

            println!("{}{}{}{}{}", value.to_owned(), &colored_bar, &empty, &count_str, &percentage_str);
        }
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