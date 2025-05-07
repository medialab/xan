<!-- Generated -->
# xan split

```txt
Splits the given CSV data into chunks.

The files are written to the directory given with the name '{start}.csv',
where {start} is the index of the first record of the chunk (starting at 0).

Usage:
    xan split [options] <outdir> [<input>]
    xan split --help

split options:
    -s, --size <arg>       The number of records to write into each chunk.
                           [default: 4096]
    -c, --chunks <n>       Divide the file into approximately <n> chunks having
                           roughly the same number of records. Target file must be
                           seekable (e.g. this will not work with stdin nor gzipped
                           files).
    --segments             When used with -c/--chunks, output the byte offsets of
                           found segments insteads.
    --filename <filename>  A filename template to use when constructing
                           the names of the output files.  The string '{}'
                           will be replaced by a value based on the value
                           of the field, but sanitized for shell safety.
                           [default: {}.csv]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Otherwise, the first row will
                           appear in all chunks as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
