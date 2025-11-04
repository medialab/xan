use rand::seq::SliceRandom;
use std::io::SeekFrom;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Shuffle the given CSV file. Requires memory proportional to the
number of rows of the file (approx. 2 u64 per row).

Note that rows from input file are copied as-is in the output.
This means that no CSV serialization harmonization will happen,
unless --in-memory is set.

Also, since this command needs random access in the input file, it
does not work with stdin or piping (unless --in-memory) is set.

Usage:
    xan shuffle [options] [<input>]
    xan shuffle --help

shuffle options:
    --seed <number>        RNG seed.
    -m, --in-memory        Load all CSV data in memory before shuffling it. Can
                           be useful for streamed inputs such as stdin but of
                           course costs more memory.

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
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_seed: Option<usize>,
    flag_in_memory: bool,
}

fn run_random_access(args: Args) -> CliResult<()> {
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);
    let wconf = Config::new(&args.flag_output);

    // Seeding rng
    let mut rng = util::acquire_rng(args.flag_seed);

    let mut header_len: Option<usize> = None;
    let mut positions: Vec<(u64, usize)> = Vec::new();
    let mut last_pos: u64 = 0;

    let mut output_wtr = wconf.io_writer()?;

    {
        let mut rdr = rconf.reader()?;

        if !rconf.no_headers {
            let header = rdr.byte_headers()?;

            if !header.is_empty() {
                last_pos = rdr.position().byte();
                header_len = Some(last_pos as usize);
            }
        }

        let mut record = csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            let pos = rdr.position().byte();
            positions.push((last_pos, (pos - last_pos) as usize));
            last_pos = pos;
        }

        positions.shuffle(&mut rng);
    }

    let mut input_rdr = rconf.io_reader_for_random_access()?;
    let mut reading_buffer: Vec<u8> = Vec::new();

    if let Some(l) = header_len {
        reading_buffer.try_reserve(l).expect("not enough memory");
        reading_buffer.extend((reading_buffer.len()..l).map(|_| 0));

        input_rdr.read_exact(&mut reading_buffer[0..l])?;
        output_wtr.write_all(&reading_buffer[0..l])?;
    }

    for (byte_offset, l) in positions {
        input_rdr.seek(SeekFrom::Start(byte_offset))?;

        reading_buffer.try_reserve(l).expect("not enough memory");
        reading_buffer.extend((reading_buffer.len()..l).map(|_| 0));

        input_rdr.read_exact(&mut reading_buffer[0..l])?;
        output_wtr.write_all(&reading_buffer[0..l])?;
    }

    Ok(output_wtr.flush()?)
}

fn run_in_memory(args: Args) -> CliResult<()> {
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);
    let wconf = Config::new(&args.flag_output);

    // Seeding rng
    let mut rng = util::acquire_rng(args.flag_seed);

    let mut rdr = rconf.reader()?;
    let mut wtr = wconf.writer()?;

    if !rconf.no_headers {
        wtr.write_byte_record(rdr.byte_headers()?)?;
    }

    let mut rows: Vec<csv::ByteRecord> = Vec::new();

    for record in rdr.into_byte_records() {
        rows.push(record.unwrap());
    }

    rows.shuffle(&mut rng);

    for record in rows {
        wtr.write_byte_record(&record)?;
    }

    Ok(wtr.flush()?)
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_in_memory {
        run_in_memory(args)
    } else {
        run_random_access(args)
    }
}
