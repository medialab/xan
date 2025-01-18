use crate::config::{Config, Delimiter};
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

static USAGE: &str = "
Enumerate a CSV file by preprending an index column to each row.

Alternatively prepend a byte offset column instead when using
the -B, --byte-offset flag.

Usage:
    xan enum [options] [<input>]
    xan enum --help

enum options:
    -c, --column-name <arg>  Name of the column to prepend. Will default to \"index\",
                             or \"byte_offset\" when -B, --byte-offset is given.
    -S, --start <arg>        Number to count from. [default: 0].
    -B, --byte-offset        Whether to indicate the byte offset of the row
                             in the file instead. Can be useful to perform
                             constant time slicing with `xan slice --byte-offset`
                             later on.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not considered as being
                           the file header.
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_start: i64,
    flag_column_name: Option<String>,
    flag_byte_offset: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = conf.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    if !args.flag_no_headers {
        let column_name = args.flag_column_name.unwrap_or(
            (if args.flag_byte_offset {
                "byte_offset"
            } else {
                "index"
            })
            .to_string(),
        );

        let headers = rdr.byte_headers()?.prepend(column_name.as_bytes());

        wtr.write_byte_record(&headers)?;
    }

    let mut record = csv::ByteRecord::new();
    let mut counter = args.flag_start;

    while rdr.read_byte_record(&mut record)? {
        let new_record = if args.flag_byte_offset {
            record.prepend(record.position().unwrap().byte().to_string().as_bytes())
        } else {
            record.prepend(counter.to_string().as_bytes())
        };

        wtr.write_byte_record(&new_record)?;

        counter += 1;
    }

    Ok(wtr.flush()?)
}
