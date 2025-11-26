<!-- Generated -->
# xan reverse

```txt
Reverse rows of CSV data.

If target is seekable (e.g. an uncompressed file on disk), this command is
able to work in amortized linear time and constant memory. If target is not
seekable, this command will need to buffer the whole file into memory to
be able to reverse it.

If you only need to retrieve the last rows of a large file, see `xan tail`
or `xan slice -L` instead.

Usage:
    xan reverse [options] [<input>]

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
