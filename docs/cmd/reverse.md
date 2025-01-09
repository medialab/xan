<!-- Generated -->
# xan reverse

```txt
Reverse rows of CSV data.

Useful to retrieve the last lines of a large file for instance, or for cases when
there is no column that can be used for sorting in reverse order, or when keys are
not unique and order of rows with the same key needs to be preserved.

This function is memory efficient by default but only for seekable inputs (ones with
the possibility to randomly access data, e.g. a file on disk, but not a piped stream).
Others sources need to be read using --in-memory flag and will need to load full
data into memory unfortunately.

Usage:
    xan reverse [options] [<input>]

reverse options:
    -m, --in-memory        Load all CSV data in memory before reversing it. Can
                           be useful for streamed inputs such as stdin but at the
                           expense of memory.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Namely, it will be reversed with the rest
                           of the rows. Otherwise, the first row will always
                           appear as the header row in the output.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
