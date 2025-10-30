<!-- Generated -->
# xan input

```txt
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
    --tabs            Same as -d '\t', i.e. use tabulations as delimiter.
    --quote <char>    The quote character to use. [default: "]
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
    --vcf  Indicate that the given stream should be understood as a VCF ("Variant Call Format")
           file from bioinformatics. This is not needed when using xan on a file
           with `.vcf` extension because xan already knows how to handle them.
           https://en.wikipedia.org/wiki/Variant_Call_Format
    --gtf  Indicate that the given stream should be understood as a GTF ("Gene Transfer Format")
           file from bioinformatics. This is not needed when using xan on a file
           with `.gtf` or `.gff2` extension because xan already knows how to handle them.
           https://en.wikipedia.org/wiki/Gene_transfer_format
    --gff  Indicate that the given stream should be understood as a GFF ("General Feature Format")
           file from bioinformatics. This flag is implied if target file has
           the `.gff` or `.gff3` extension.
           https://en.wikipedia.org/wiki/General_feature_format
    --sam  Indicate that the given stream should be understood as a SAM ("Sequence Alignment Map")
           file from bioinformatics. This is not needed when using xan on a file
           with `.sam` extension because xan already knows how to handle them.
           https://en.wikipedia.org/wiki/SAM_(file_format)
    --bed  Indicate that the given stream should be understood as a BED ("Browser Extensible Data")
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
```
