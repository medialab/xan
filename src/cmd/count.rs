use std::num::{NonZeroU64, NonZeroUsize};

use crate::cmd::parallel::Args as ParallelArgs;
use crate::config::{Config, Delimiter};
use crate::read::sample_initial_records;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Prints a count of the number of records in the CSV data.

Note that the count will not include the header row (unless --no-headers is
given).

You can also use the -p/--parallel or -t/--threads flag to count the number
of records of the file in parallel to go faster. But this cannot work on streams
or gzipped files, unless a `.gzi` index (as created by `bgzip -i`) can be found
beside it.

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
    -z, --zero-copy
    -m, --mmap

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
    flag_threads: Option<NonZeroUsize>,
    flag_approx: bool,
    flag_sample_size: NonZeroU64,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_zero_copy: bool,
    flag_mmap: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_zero_copy || args.flag_mmap {
        use crate::altered_csv_core::ReadRecordResult;
        use memmap2::Mmap;
        use std::io::{BufRead, BufReader};

        let mut csv_reader = crate::altered_csv_core::ReaderBuilder::new().build();
        // let mut out_buffer = vec![0; 1024 * (1 << 10)];
        // let mut ends = vec![0usize; 1024 * (1 << 10)];

        let mut count: u64 = 0;

        // mmap
        if args.flag_mmap {
            dbg!("mmap");
            let file = std::fs::File::open(&args.arg_input.unwrap())?;
            let map = unsafe { Mmap::map(&file).unwrap() };

            let mut i: usize = 0;

            loop {
                let input = &map[i..];

                let (result, nin, _) = csv_reader.read_record(input);

                i += nin;

                match result {
                    ReadRecordResult::End => break,
                    ReadRecordResult::InputEmpty => continue,
                    ReadRecordResult::OutputEndsFull | ReadRecordResult::OutputFull => todo!(),
                    ReadRecordResult::Record => {
                        count += 1;
                    }
                }
            }

            println!("{}", count);

            return Ok(());
        }

        // zero-copy
        dbg!("zero-copy");
        let mut reader = BufReader::with_capacity(
            1024 * (1 << 10),
            Config::new(&args.arg_input.clone()).io_reader()?,
        );

        loop {
            let input = reader.fill_buf()?;
            let (result, nin, _) = csv_reader.read_record(input);

            reader.consume(nin);

            match result {
                ReadRecordResult::End => break,
                ReadRecordResult::InputEmpty => continue,
                ReadRecordResult::OutputEndsFull | ReadRecordResult::OutputFull => todo!(),
                ReadRecordResult::Record => {
                    count += 1;
                }
            }
        }

        println!("{}", count);

        return Ok(());
    }

    dbg!("regular");

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_approx {
            Err("-p/--parallel or -t/--threads cannot be used with -a/--approx!")?;
        }

        let mut parallel_args = ParallelArgs::single_file(&args.arg_input, args.flag_threads)?;

        parallel_args.cmd_count = true;

        parallel_args.flag_no_headers = args.flag_no_headers;
        parallel_args.flag_output = args.flag_output;
        parallel_args.flag_delimiter = args.flag_delimiter;

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
