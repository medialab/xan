use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Reverse rows of CSV data.

If target is seekable (e.g. an uncompressed file on disk), this command is
able to work in amortized linear time and constant memory. If target is not
seekable, this command will need to buffer the whole file into memory to
be able to reverse it.

If you only need to retrieve the last rows of a large file, see `xan tail`
or `xan slice -L` instead.

Usage:
    xan reverse [options] [<input>]

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
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconfig = &mut Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if let Ok(mut reverse_reader) = rconfig.reverse_reader() {
        let headers = reverse_reader.byte_headers();

        if !rconfig.no_headers && !headers.is_empty() {
            wtr.write_byte_record(headers)?;
        }

        let mut record = simd_csv::ByteRecord::new();

        while reverse_reader.read_byte_record(&mut record)? {
            wtr.write_byte_record(&record)?;
        }
    } else {
        let mut reader = rconfig.simd_reader()?;
        let records = reader.byte_records().collect::<Result<Vec<_>, _>>()?;

        if !rconfig.no_headers {
            wtr.write_byte_record(reader.byte_headers()?)?;
        }

        for record in records.into_iter().rev() {
            wtr.write_byte_record(&record)?;
        }
    }

    Ok(wtr.flush()?)
}
