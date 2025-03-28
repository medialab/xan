use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Read unusually formatted CSV data.

This means being able to process CSV data with peculiar quoting rules
using --quote or --no-quoting, or dealing with character escaping with --escape.

This command also makes it possible to process CSV files containing metadata and
headers before the tabular data itself, with -S/--skip-headers or --vcf.

Usage:
    xan input [options] [<input>]

input options:
    --tabs                        Same as -d '\\t', i.e. use tabulations as delimiter.
    --quote <char>                The quote character to use. [default: \"]
    --escape <char>               The escape character to use. When not specified,
                                  quotes are escaped by doubling them.
    --no-quoting                  Disable quoting completely.
    -S, --skip-headers <pattern>  Skip header lines starting with the given pattern.
    --vcf                         Process a \"Variant Call Format\" tabular file with headers.
                                  A shorthand for --tabs -S '##' and some processing over the
                                  first column name: https://fr.wikipedia.org/wiki/Variant_Call_Format

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
    flag_vcf: bool,
    flag_escape: Option<Delimiter>,
    flag_no_quoting: bool,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_vcf {
            self.flag_tabs = true;
            self.flag_skip_headers = Some("##".to_string());
        }

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

                    if args.flag_vcf {
                        row = row
                            .iter()
                            .enumerate()
                            .map(|(i, cell)| {
                                if i == 0 && cell == b"#CHROM" {
                                    b"CHROM"
                                } else {
                                    cell
                                }
                            })
                            .collect();
                    }
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
