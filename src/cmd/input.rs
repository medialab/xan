use regex::bytes::Regex;

use crate::config::{Compression, Config, Delimiter};
use crate::read;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Read unusually formatted CSV data.

This means being able to process CSV data with peculiar quoting rules
using --quote or --no-quoting, or dealing with character escaping, typically
files with backslash escaping, with --escape.

This command is also able to skip metadata headers sometimes found at the beginning
of CSV-adjacent formats with the -L/--skip-lines, -U/--skip-until & -W/--skip-while
flags.

Finally you can also use this command to handle compressed streams and well-known
CSV-adjacent format streams (note that it is not necessary to use `xan input` if
the file is already on disk and has the expected extension, as xan knows how
to deal with some of those formats out-of-the-box). A notable exception to this is
GFF files that require `xan input` to be read.

Usage:
    xan input [options] [<input>]

formatting options:
    --tabs            Same as -d '\\t', i.e. use tabulations as delimiter.
    --quote <char>    The quote character to use. [default: \"]
    --escape <char>   The escape character to use. When not specified,
                      quotes are escaped by doubling them.
    --no-quoting      Disable quoting completely.
    --comment <char>  Skip records starting with this character.
    --trim            Whether to trim cell values.

header skipping options:
    -L, --skip-lines <n>        Skip the first <n> lines of the file.
    -U, --skip-until <pattern>  Skip lines until <pattern> matches.
    -W, --skip-while <pattern>  Skip lines while <pattern> matches.

CSV-adjacent data format options:
    --vcf  Indicate that the given stream should be understood as a VCF (\"Variant Call Format\")
           file from bioinformatics. This is not needed when using xan on a file
           with `.vcf` extension because xan already knows how to handle them.
           https://en.wikipedia.org/wiki/Variant_Call_Format
    --gtf  Indicate that the given stream should be understood as a GTF (\"Gene Transfer Format\")
           file from bioinformatics. This is not needed when using xan on a file
           with `.gtf` or `.gff2` extension because xan already knows how to handle them.
           https://en.wikipedia.org/wiki/Gene_transfer_format
    --gff  Indicate that the given stream should be understood as a GFF (\"General Feature Format\")
           file from bioinformatics. This flag is implied if target file has
           the `.gff` or `.gff3` extension.
           https://en.wikipedia.org/wiki/General_feature_format
    --sam  Indicate that the given stream should be understood as a SAM (\"Sequence Alignment Map\")
           file from bioinformatics. This is not needed when using xan on a file
           with `.sam` extension because xan already knows how to handle them.
           https://en.wikipedia.org/wiki/SAM_(file_format)
    --bed  Indicate that the given stream should be understood as a BED (\"Browser Extensible Data\")
           file from bioinformatics. This is not needed when using xan on a file
           with `.bed` extension because xan already knows how to handle them.
           Note that the file will be considered as tab-delimited, not space-delimited!
           https://en.wikipedia.org/wiki/BED_(file_format)
    --cdx  Indicate that the given stream should be understood as a CDX index
           file from web archives. This is not needed when using xan on a file
           with `.cdx` extension because xan already knows how to handle them.
           https://iipc.github.io/warc-specifications/specifications/cdx-format/cdx-2015/

compression options:
    --gzip  Read a gzip-compressed stream or gzip-compressed file without the
            standard `.gz` extension.
    --zstd  Read a zstd-compressed stream or zstd-compressed file without the
            standard `.zst` extension.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_input: Option<String>,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_tabs: bool,
    flag_comment: Option<Delimiter>,
    flag_trim: bool,
    flag_quote: Delimiter,
    flag_escape: Option<Delimiter>,
    flag_no_quoting: bool,
    flag_skip_lines: Option<usize>,
    flag_skip_until: Option<String>,
    flag_skip_while: Option<String>,
    flag_gzip: bool,
    flag_zstd: bool,
    flag_vcf: bool,
    flag_gtf: bool,
    flag_gff: bool,
    flag_sam: bool,
    flag_bed: bool,
    flag_cdx: bool,
}

impl Args {
    fn resolve(&mut self) {
        if let Some(path) = self.arg_input.as_ref() {
            if path.contains(".gff") {
                self.flag_gff = true;
            }
        }

        if self.flag_gff {
            self.flag_tabs = true;
            self.flag_comment = Some(Delimiter(b'#'));
        }

        if self.flag_tabs {
            self.flag_delimiter = Some(Delimiter(b'\t'));
        }
    }

    fn can_use_simd(&self) -> bool {
        self.flag_comment.is_none()
            && !self.flag_trim
            && self.flag_escape.is_none()
            && !self.flag_no_quoting
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    let formats = args.flag_vcf as u8
        + args.flag_gtf as u8
        + args.flag_gff as u8
        + args.flag_sam as u8
        + args.flag_bed as u8
        + args.flag_cdx as u8;

    if formats > 1 {
        Err("can only select one of --vcf, --gtf, -gff, --bed, --sam & --cdx!")?;
    }

    if args.flag_gzip && args.flag_zstd {
        Err("can only select one of --gzip & --zstd!")?;
    }

    let skippers = args.flag_skip_lines.is_some() as u8
        + args.flag_skip_until.is_some() as u8
        + args.flag_skip_while.is_some() as u8;

    if skippers > 1 {
        Err("can only select one of -L/--skip-lines, -W,--skip-while & -U,--skip-until!")?;
    }

    let pretend_path = if args.flag_vcf {
        Some("file.vcf")
    } else if args.flag_gtf {
        Some("file.gtf")
    } else if args.flag_sam {
        Some("file.sam")
    } else if args.flag_bed {
        Some("file.bed")
    } else if args.flag_cdx {
        Some("file.cdx")
    } else {
        None
    };

    let mut rconfig = Config::with_pretend_path(&args.arg_input, pretend_path)
        .delimiter(args.flag_delimiter)
        .no_headers(true)
        .quote(args.flag_quote.as_byte())
        .quoting(!args.flag_no_quoting)
        .comment(args.flag_comment.map(Delimiter::as_byte))
        .escape(args.flag_escape.map(Delimiter::as_byte))
        .trim(args.flag_trim);

    if args.flag_gzip {
        rconfig.set_compression(Compression::Gzip);
    } else if args.flag_zstd {
        rconfig.set_compression(Compression::Zstd);
    }

    let wconfig = Config::new(&args.flag_output);
    let mut wtr = wconfig.simd_writer()?;

    // Skipping header lines?
    let io_reader = if let Some(limit) = args.flag_skip_lines {
        read::consume_lines(rconfig.io_reader()?, limit)?
            .ok_or_else(|| format!("-L/--skip-lines {}: not enough lines to skip!", limit))
            .map(|(_, r)| Box::new(r))?
    } else if let Some(pattern) = args.flag_skip_until.as_ref() {
        let pattern = Regex::new(pattern)?;

        read::consume_header_until(rconfig.io_reader()?, &pattern)?
            .ok_or_else(|| format!("-U/--skip-until {}: skipped everything!", pattern))
            .map(|(_, r)| Box::new(r))?
    } else if let Some(pattern) = args.flag_skip_while.as_ref() {
        let pattern = Regex::new(pattern)?;

        read::consume_header_while(rconfig.io_reader()?, &pattern)?
            .ok_or_else(|| format!("-U/--skip-while {}: skipped everything!", pattern))
            .map(|(_, r)| Box::new(r))?
    } else {
        rconfig.io_reader()?
    };

    if args.can_use_simd() {
        let mut rdr = rconfig.simd_csv_reader_from_reader(io_reader);
        let mut record = simd_csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            wtr.write_byte_record(&record)?;
        }
    } else {
        let mut rdr = rconfig.csv_reader_from_reader(io_reader);
        let mut record = csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            wtr.write_record(&record)?;
        }
    }

    Ok(wtr.flush()?)
}
