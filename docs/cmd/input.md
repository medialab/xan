<!-- Generated -->
# xan input

```txt
Read unusually formatted CSV data.

This means being able to process CSV data with peculiar quoting rules
using --quote or --no-quoting, or dealing with character escaping with --escape.

This command also makes it possible to process CSV files containing metadata and
headers before the tabular data itself, with -S/--skip-headers, -L/--skip-lines.

This command also recognizes variant of TSV files from bioinformatics out of the
box, either by detecting their extension or through dedicated flags:

    - VCF ("Variant Call Format") files:
        extensions: `.vcf`, `.vcf.gz`
        flag: --vcf
        reference: https://en.wikipedia.org/wiki/Variant_Call_Format
    - GTF ("Gene Transfert Format") files:
        extension: `.gtf`, `.gtf.gz`, `.gff2`, `.gff2.gz`
        flag: --gtf
        reference: https://en.wikipedia.org/wiki/Gene_transfer_format
    - GFF ("General Feature Format") files:
        extension: `.gff`, `.gff.gz`, `.gff3`, `.gff3.gz`
        flag: --gff
        reference: https://en.wikipedia.org/wiki/General_feature_format

Usage:
    xan input [options] [<input>]

input options:
    --tabs                        Same as -d '\t', i.e. use tabulations as delimiter.
    --quote <char>                The quote character to use. [default: "]
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
```
