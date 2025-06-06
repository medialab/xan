<!-- Generated -->
# xan range

```txt
Create a CSV file with one column representing a numerical range. This is mostly
useful when piping to the `map`, `transform` or `select -e` command to easily
generate CSV files from scratch.

By default, the output column will be named "n" but can be renamed using
the -c, --column-name flag.

Note that like in most programming language, the end of the range is exclusive,
but can be included with -i, --inclusive.

Example:

    Creating a range of urls files by piping `range` into `transform`:
        $ xan range 100 | xan select -e '"somewebsite.com?id=" ++ n as url'

Usage:
    xan range [options] <end> [<input>]
    xan range --help

range options:
    -s, --start <n>           Start of the range. [default: 0]
    --step <n>                Step of the range. [default: 1]
    -c, --column-name <name>  Name of the column containing the range.
                              [default: n]
    -i, --inclusive           Include the end bound.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
```
