use regex::bytes::Regex;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Read unusually formatted CSV data.

This means being able to process CSV data with peculiar quoting rules
using --quote or --no-quoting, or dealing with character escaping with --escape.

This command also makes it possible to process CSV files containing metadata and
headers before the tabular data itself, with -S/--skip-headers, -L/--skip-lines.

This command also recognizes variant of TSV files from bioinformatics out of the
box, either by detecting their extension or through dedicated flags:

    - VCF (\"Variant Call Format\") files:
        extensions: `.vcf`, `.vcf.gz`
        flag: --vcf
        reference: https://en.wikipedia.org/wiki/Variant_Call_Format
    - GTF (\"Gene Transfert Format\") files:
        extension: `.gtf`, `.gtf.gz`, `.gff2`, `.gff2.gz`
        flag: --gtf
        reference: https://en.wikipedia.org/wiki/Gene_transfer_format
    - GFF (\"General Feature Format\") files:
        extension: `.gff`, `.gff.gz`, `.gff3`, `.gff3.gz`
        flag: --gff
        reference: https://en.wikipedia.org/wiki/General_feature_format

Usage:
    xan input [options] [<input>]

input options:
    --tabs                        Same as -d '\\t', i.e. use tabulations as delimiter.
    --quote <char>                The quote character to use. [default: \"]
    --escape <char>               The escape character to use. When not specified,
                                  quotes are escaped by doubling them.
    --no-quoting                  Disable quoting completely.
    -L, --skip-lines <n>          Skip the first <n> lines of the file.
    -H, --skip-headers <pattern>  Skip header lines matching the given regex pattern.
    -R, --skip-rows <pattern>     Skip rows matching the given regex pattern.
    --vcf                         Process a VCF file. Shorthand for --tabs -H '^##' and
                                  some processing over the first column name.
    --gtf                         Process a GTF file. Shorthand for --tabs -H '^#!'.
    --gff                         Process a GFF file. Shorthand for --tabs -H '^#[#!]'
                                  and -R '^###$'.

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
    flag_skip_lines: Option<usize>,
    flag_skip_headers: Option<String>,
    flag_skip_rows: Option<String>,
    flag_vcf: bool,
    flag_gtf: bool,
    flag_gff: bool,
    flag_escape: Option<Delimiter>,
    flag_no_quoting: bool,
}

impl Args {
    fn resolve(&mut self) {
        if let Some(path) = &self.arg_input {
            if path.ends_with(".vcf") || path.ends_with(".vcf.gz") {
                self.flag_vcf = true;
            }

            if path.ends_with(".gtf")
                || path.ends_with(".gtf.gz")
                || path.ends_with(".gff2")
                || path.ends_with(".gff2.gz")
            {
                self.flag_gtf = true;
            }

            if path.ends_with(".gff")
                || path.ends_with(".gff.gz")
                || path.ends_with(".gff3")
                || path.ends_with(".gff3.gz")
            {
                self.flag_gff = true;
            }
        }

        if self.flag_vcf {
            self.flag_tabs = true;
            self.flag_skip_headers = Some("^##".to_string());
        }

        if self.flag_gtf {
            self.flag_tabs = true;
            self.flag_skip_headers = Some("^#!".to_string());
        }

        if self.flag_gff {
            self.flag_tabs = true;
            self.flag_skip_headers = Some("^#[#!]".to_string());
            self.flag_skip_rows = Some("^###$".to_string());
        }

        if self.flag_tabs {
            self.flag_delimiter = Some(Delimiter(b'\t'));
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    if args.flag_skip_headers.is_some() && args.flag_skip_lines.is_some() {
        Err("-L/--skip-lines does not work with -H/--skip-headers!")?;
    }

    let mut rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(true)
        .flexible(
            args.flag_skip_headers.is_some()
                || args.flag_skip_lines.is_some()
                || args.flag_skip_rows.is_some(),
        )
        .quote(args.flag_quote.as_byte());

    let skip_headers = args
        .flag_skip_headers
        .as_ref()
        .map(|p| Regex::new(p))
        .transpose()?;

    let skip_rows = args
        .flag_skip_rows
        .as_ref()
        .map(|p| Regex::new(p))
        .transpose()?;

    let wconfig = Config::new(&args.flag_output);

    if let Some(escape) = args.flag_escape {
        rconfig = rconfig.escape(Some(escape.as_byte())).double_quote(false);
    }
    if args.flag_no_quoting {
        rconfig = rconfig.quoting(false);
    }

    let mut wtr = wconfig.writer()?;
    let mut record = csv::ByteRecord::new();

    let mut rdr = rconfig.reader()?;
    let mut headers_have_been_skipped = false;
    let mut i: usize = 0;

    while rdr.read_byte_record(&mut record)? {
        i += 1;

        if let Some(pattern) = &skip_headers {
            if !headers_have_been_skipped {
                if !pattern.is_match(record.as_slice()) {
                    headers_have_been_skipped = true;

                    if args.flag_vcf {
                        record = record
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
        } else if let Some(skip_lines) = args.flag_skip_lines {
            if i <= skip_lines {
                continue;
            }
        }

        if let Some(pattern) = &skip_rows {
            if pattern.is_match(record.as_slice()) {
                continue;
            }
        }

        wtr.write_record(&record)?;
    }

    wtr.flush()?;

    Ok(())
}
