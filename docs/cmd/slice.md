<!-- Generated -->
# xan slice

```txt
Returns the rows in the range specified (starting at 0, half-open interval).
The range does not include headers.

If the start of the range isn't specified, then the slice starts from the first
record in the CSV data.

If the end of the range isn't specified, then the slice continues to the last
record in the CSV data.

This operation can be made much faster by creating an index with 'xan index'
first. Namely, a slice on an index requires parsing just the rows that are
sliced. Without an index, all rows up to the first row in the slice must be
parsed.

Usage:
    xan slice [options] [<input>]

slice options:
    -s, --start <n>  The index of the record to slice from.
    -e, --end <n>    The index of the record to slice to.
    -l, --len <n>    The length of the slice (can be used instead
                     of --end).
    -i, --index <i>  Slice a single record (shortcut for -s N -l 1).
                     You can also provide multiples indices separated by
                     commas, e.g. "1,4,67,89". Note that selected records
                     will be emitted in file order.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Otherwise, the first row will always
                           appear in the output as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
