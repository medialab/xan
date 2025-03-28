use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Read unusually formatted CSV data.

Generally, all xan commands support basic options like specifying the delimiter
used in CSV data. This does not cover all possible types of CSV data. For
example, some CSV files don't use `\"` for quotes or use different escaping
styles.

Usage:
    xan input [options] [<input>]

input options:
    --tabs                        Same as -d '\t', i.e. use tabulations as delimiter.
    --quote <char>                The quote character to use. [default: \"]
    --escape <char>               The escape character to use. When not specified,
                                  quotes are escaped by doubling them.
    --no-quoting                  Disable quoting completely.
    -S, --skip-headers <pattern>  Skip header lines starting with the given pattern.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_tabs: bool,
    flag_quote: Delimiter,
    flag_skip_headers: Option<String>,
    flag_escape: Option<Delimiter>,
    flag_no_quoting: bool,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_tabs {
            self.flag_delimiter = Some(Delimiter(b'\t'));
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    let mut rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(true)
        .flexible(args.flag_skip_headers.is_some())
        .quote(args.flag_quote.as_byte());

    let wconfig = Config::new(&args.flag_output);

    if let Some(escape) = args.flag_escape {
        rconfig = rconfig.escape(Some(escape.as_byte())).double_quote(false);
    }
    if args.flag_no_quoting {
        rconfig = rconfig.quoting(false);
    }

    let mut wtr = wconfig.writer()?;
    let mut row = csv::ByteRecord::new();

    let mut rdr = rconfig.reader()?;
    let mut headers_have_been_skipped = false;

    while rdr.read_byte_record(&mut row)? {
        if let Some(pattern) = &args.flag_skip_headers {
            if !headers_have_been_skipped {
                if !row[0].starts_with(pattern.as_bytes()) {
                    headers_have_been_skipped = true;
                } else {
                    continue;
                }
            }
        }

        wtr.write_record(&row)?;
    }

    wtr.flush()?;

    Ok(())
}
