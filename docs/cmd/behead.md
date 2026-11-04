<!-- Generated -->
# xan behead

```txt
Drop a CSV file's header.

Usage:
    xan behead [options] [<input>]
    xan guillotine [options] [<input>]

behead options:
    -A, --append  Only drop headers if output already exists and
                  is not empty. Requires -o/--output to be set.
    --raw         Drop the header and flush the rest of the file to output as-is
                  without standardizing CSV format in order to go faster.
                  This is activated by default if input is already standard
                  CSV (commas, double quotes).

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
