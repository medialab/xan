<!-- Generated -->
# xan input

```txt
Read unusually formatted CSV data.

Generally, all xan commands support basic options like specifying the delimiter
used in CSV data. This does not cover all possible types of CSV data. For
example, some CSV files don't use `"` for quotes or use different escaping
styles.

Usage:
    xan input [options] [<input>]

input options:
    --tabs           Same as -d '	', i.e. use tabulations as delimiter.
    --quote <char>   The quote character to use. [default: "]
    --escape <char>  The escape character to use. When not specified,
                     quotes are escaped by doubling them.
    --no-quoting     Disable quoting completely.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
