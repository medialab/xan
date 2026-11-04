use std::fs::OpenOptions;
use std::io::{self, Write};

use simd_csv::ByteRecord;

use crate::CliResult;
use crate::config::{Config, Delimiter};
use crate::utils;

static USAGE: &str = "
Drop a CSV file's header.

Usage:
    xan behead [options] [<input>]
    xan guillotine [options] [<input>]

behead options:
    -A, --append  Only drop headers if output already exists and
                  is not empty. Requires -o/--output to be set.
    --raw         Drop the header and flush the rest of the file to the output
                  as-is without standardizing CSV format in order to go faster.
                  This is activated by default if input is already standard
                  CSV (commas, double quotes).

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_append: bool,
    flag_raw: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = utils::get_args(USAGE, argv)?;

    if args.flag_append && args.flag_output.is_none() {
        Err(
            "-A/--append needs to know where the output will be written!\nPlease provide -o/--output.",
        )?;
    }

    let wconf = Config::new(&args.flag_output);

    let mut actually_behead = true;

    if args.flag_append {
        let output_path = wconf.path.as_ref().unwrap();

        if !output_path.is_file() || output_path.metadata()?.len() == 0 {
            actually_behead = false;
        }
    }

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(!actually_behead);

    let mut peeker = rconf.simd_peeker()?;
    peeker.peek()?;

    let mut wtr = wconf.buf_io_writer_with_options(
        OpenOptions::new()
            .write(true)
            .create(true)
            .append(args.flag_append),
    )?;

    if args.flag_raw || rconf.is_standard_csv() {
        io::copy(&mut peeker.into_reader(), &mut wtr)?;

        Ok(wtr.flush()?)
    } else {
        let mut rdr = rconf
            .no_headers(true)
            .simd_csv_reader_from_reader(peeker.into_reader());
        let mut wtr = wconf.simd_csv_writer_from_writer(wtr);
        let mut record = ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            wtr.write_byte_record(&record)?;
        }

        Ok(wtr.flush()?)
    }
}
