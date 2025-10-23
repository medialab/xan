<!-- Generated -->
# xan behead

```txt
Drop a CSV file's header.

Note that this command does not try to be clever and only parses the first CSV
row encountered to drop it. The rest of the file will be redirected to the output
as-is without any kind of normalization.

Usage:
    xan behead [options] [<input>]
    xan guillotine [options] [<input>]

behead options:
    -A, --append  Only drop headers if output already exists and
                  is not empty. Requires -o/--output to be set.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
