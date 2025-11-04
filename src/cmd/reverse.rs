use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Reverse rows of CSV data.

Useful to retrieve the last lines of a large file for instance, or for cases when
there is no column that can be used for sorting in reverse order, or when keys are
not unique and order of rows with the same key needs to be preserved.

This function is memory efficient by default but only for seekable inputs (ones with
the possibility to randomly access data, e.g. a file on disk, but not a piped stream).
Others sources need to be read using --in-memory flag and will need to load full
data into memory unfortunately.

Usage:
    xan reverse [options] [<input>]

reverse options:
    -m, --in-memory        Load all CSV data in memory before reversing it. Can
                           be useful for streamed inputs such as stdin but at the
                           expense of memory.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Namely, it will be reversed with the rest
                           of the rows. Otherwise, the first row will always
                           appear as the header row in the output.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_in_memory: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconfig = &mut Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    if args.flag_in_memory {
        run_without_memory_efficiency(rconfig, args)
    } else {
        run_with_memory_efficiency(rconfig, args)
    }
}

fn run_with_memory_efficiency(rconfig: &mut Config, args: Args) -> CliResult<()> {
    let mut reverse_reader = rconfig.reverse_reader().map_err(|_| {
        "can't use provided input: needs to be loaded in the RAM using -m, --in-memory flag"
    })?;

    let headers = reverse_reader.byte_headers();

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if !rconfig.no_headers && !headers.is_empty() {
        wtr.write_byte_record(headers)?;
    }

    let mut record = simd_csv::ByteRecord::new();

    while reverse_reader.read_byte_record(&mut record)? {
        wtr.write_byte_record(&record)?;
    }

    Ok(wtr.flush()?)
}

fn run_without_memory_efficiency(rconfig: &mut Config, args: Args) -> CliResult<()> {
    let mut reader = rconfig.reader()?;
    let all = reader.byte_records().collect::<Result<Vec<_>, _>>()?;

    let mut wtr = Config::new(&args.flag_output).writer()?;
    rconfig.write_headers(&mut reader, &mut wtr)?;

    for r in all.into_iter().rev() {
        wtr.write_byte_record(&r)?;
    }

    Ok(wtr.flush()?)
}
