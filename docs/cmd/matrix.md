<!-- Generated -->
# xan matrix

```txt
Convert CSV data to matrix data.

Supported modes:
    corr: convert a selection of columns into a full
          correlation matrix.

Usage:
    xan matrix corr [options] [<input>]
    xan matrix --help

matrix corr options:
    -s, --select <columns>  Columns to consider for the correlation
                            matrix.
    -D, --fill-diagonal     Whether to fill diagonal with ones.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
```
