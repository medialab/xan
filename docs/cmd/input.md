<!-- Generated -->
# xan input

```txt
Read unusually formatted CSV data.

This means being able to process CSV data with peculiar quoting rules
using --quote or --no-quoting, or dealing with character escaping with --escape.

This command also makes it possible to process CSV files containing metadata and
headers before the tabular data itself, with -S/--skip-headers, -L/--skip-lines.

This command is also able to recognize VCF files, from bioinformatics, out of the
box, either when the command is given a path with a `.vcf` extension or when
explicitly passing the --vcf flag.

Usage:
    xan input [options] [<input>]

input options:
    --tabs                        Same as -d '\t', i.e. use tabulations as delimiter.
    --quote <char>                The quote character to use. [default: "]
    --escape <char>               The escape character to use. When not specified,
                                  quotes are escaped by doubling them.
    --no-quoting                  Disable quoting completely.
    -L, --skip-lines <n>          Skip the first <n> lines of the file.
    -H, --skip-headers <pattern>  Skip header lines starting with the given pattern.
    --vcf                         Process a "Variant Call Format" tabular file with headers.
                                  A shorthand for --tabs -H '##' and some processing over the
                                  first column name: https://en.wikipedia.org/wiki/Variant_Call_Format
                                  Will be toggled by default if given file has a `.vcf` extension.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
