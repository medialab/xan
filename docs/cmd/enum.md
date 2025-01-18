<!-- Generated -->
# xan enum

```txt
Enumerate a CSV file by preprending an index column to each row.

Alternatively prepend a byte offset column instead when using
the -B, --byte-offset flag.

Usage:
    xan enum [options] [<input>]
    xan enum --help

enum options:
    -c, --column-name <arg>  Name of the column to prepend. Will default to "index",
                             or "byte_offset" when -B, --byte-offset is given.
    -S, --start <arg>        Number to count from. [default: 0].
    -B, --byte-offset        Whether to indicate the byte offset of the row
                             in the file instead. Can be useful to perform
                             constant time slicing with `xan slice --byte-offset`
                             later on.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not considered as being
                           the file header.
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
