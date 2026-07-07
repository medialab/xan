use std::num::NonZeroUsize;

use numfmt::{Formatter, Precision, Scales};

use crate::CliResult;
use crate::cmd::parallel::Args as ParallelArgs;
use crate::config::{Config, Delimiter};
use crate::util;

static USAGE: &str = "
Print the number of records in given CSV data.

Note that the count will not include the header row (unless --no-headers is
given).

This command uses by default a very performant CSV parser that does not even need
to find cell delimitations. This means it will not validate given CSV stream by
checking that every row has the same number of column. You can always use
the -c/--check-alignment flag to force the command to use a less performant parser
but that will perform the check.

You can also use the -p/--parallel or -t/--threads flag to count the number
of records of the file in parallel to go faster. But this cannot work on streams
or gzipped files, unless a `.gzi` index (as created by `bgzip -i`) can be found
beside it.

Usage:
    xan count [options] [<input>]

count options:
    -H, --human-readable     Format the count so it is easier to read.
    -a, --approx             Attempt to approximate a CSV file row count by sampling its
                             first rows. Target must be seekable, which means this cannot
                             work on a stream fed through stdin nor with gzipped data.
    -c, --check-alignment    Use a slower parser validating that given CSV stream yields rows
                             having the same number of columns.
    -p, --parallel           Whether to use parallelization to speed up counting.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

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
    flag_check_alignment: bool,
    flag_human_readable: bool,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_approx && args.flag_check_alignment {
        Err("-a/--approx does not work with -c/--check-alignment!")?;
    }

    if args.flag_parallel || args.flag_threads.is_some() {
        if args.flag_approx || args.flag_check_alignment {
            Err(
                "-p/--parallel or -t/--threads cannot be used with -a/--approx nor -c/--check-alignment!",
            )?;
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
        match conf.simd_seeker()? {
            None => 0,
            Some(seeker) => seeker.approx_count(),
        }
    } else if args.flag_check_alignment {
        let mut reader = conf.simd_zero_copy_reader()?;
        let mut count = 0;

        while reader.read_byte_record()?.is_some() {
            count += 1;
        }

        count
    } else {
        conf.simd_splitter()?.count_records()?
    };

    let mut writer = wconf.io_writer()?;

    if args.flag_human_readable {
        let mut si_formatter = Formatter::default()
            .scales(Scales::short())
            .precision(Precision::Decimals(1));

        writeln!(
            writer,
            "{} ({})",
            util::format_number(count),
            si_formatter.fmt2(count).replace(".0", "")
        )?;
    } else {
        writeln!(writer, "{}", count)?;
    }

    Ok(())
}
