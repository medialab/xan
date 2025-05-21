use std::num::NonZeroU64;

use crate::cmd::parallel::Args as ParallelArgs;
use crate::config::{Config, Delimiter};
use crate::read::sample_initial_records;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Prints a count of the number of records in the CSV data.

Note that the count will not include the header row (unless --no-headers is
given).

Usage:
    xan count [options] [<input>]

count options:
    -p, --parallel           Whether to use parallelization to speed up counting.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.
    -a, --approx             Attempt to approximate a CSV file row count by sampling its
                             first rows. Target must be seekable, which means this cannot
                             work on a stream fed through stdin nor with gzipped data.
    --sample-size <n>        Number of rows to sample when using -a, --approx.
                             [default: 512]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_approx: bool,
    flag_sample_size: NonZeroU64,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_approx {
            Err("-p/--parallel or -t/--threads cannot be used with -a/--approx!")?;
        }

        let mut parallel_args = ParallelArgs::single_file(&args.arg_input)?;
        parallel_args.cmd_count = true;

        return parallel_args.run();
    }

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let wconf = Config::new(&args.flag_output);

    let count = if args.flag_approx {
        let mut rdr = conf.seekable_reader()?;

        match sample_initial_records(&mut rdr, args.flag_sample_size.get())? {
            None => 0,
            Some(sample) => sample.exact_or_approx_count(),
        }
    } else {
        let mut rdr = conf.reader()?;
        let mut count = 0u64;
        let mut record = csv::ByteRecord::new();
        while rdr.read_byte_record(&mut record)? {
            count += 1;
        }
        count
    };

    let mut writer = wconf.io_writer()?;
    writeln!(writer, "{}", count)?;

    Ok(())
}
