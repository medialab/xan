<!-- Generated -->
# xan enum

```txt
Enumerate a CSV file by preprending an index column to each row.

Usage:
    xan enum [options] [<input>]
    xan enum --help

enum options:
    -c, --column-name <arg>  Name of the column to prepend. [default: index].
    -S, --start <arg>        Number to count from. [default: 0].

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not considered as being
                           the file header.
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
