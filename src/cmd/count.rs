use csv;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Prints a count of the number of records in the CSV data.

Note that the count will not include the header row (unless --no-headers is
given).

Usage:
    xan count [options] [<input>]

count options:
    --csv  Output the result as a single column, single row CSV file with
           a \"count\" header.

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
    flag_csv: bool,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let wconf = Config::new(&args.flag_output);

    let count = match conf.indexed()? {
        Some(idx) => idx.count(),
        None => {
            let mut rdr = conf.reader()?;
            let mut count = 0u64;
            let mut record = csv::ByteRecord::new();
            while rdr.read_byte_record(&mut record)? {
                count += 1;
            }
            count
        }
    };

    if args.flag_csv {
        let mut writer = wconf.writer()?;
        let mut record = csv::ByteRecord::new();
        record.push_field(b"count");
        writer.write_byte_record(&record)?;

        record.clear();
        record.push_field(format!("{}", count).as_bytes());
        writer.write_byte_record(&record)?;

        writer.flush()?;
    } else {
        let mut writer = wconf.io_writer()?;
        writeln!(writer, "{}", count)?;
    }

    Ok(())
}
