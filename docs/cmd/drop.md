<!-- Generated -->
# xan drop

```txt
Drop columns of a CSV file using the same DSL as "xan select".

Basically a shorthand for the negative selection of "xan select".

Usage:
    xan drop [options] [--] <selection> [<input>]
    xan drop --help

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
