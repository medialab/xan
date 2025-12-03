<!-- Generated -->
# xan split

```txt
Splits the given CSV data into smaller files having a fixed number of
rows given to -S, --size.

Target file can also be split into a given number of -c/--chunks.

Files will be written in current working directory by default or in any directory
given to -O/--out-dir (that will be created for your if necessary).

Usage:
    xan split [options] [<input>]
    xan split --help

split options:
    -O, --out-dir <dir>        Where to write the chunks. Defaults to current working
                               directory.
    -S, --size <arg>           The number of records to write into each chunk.
                               [default: 4096]
    -c, --chunks <n>           Divide the file into at most <n> chunks having
                               roughly the same number of records. Target file must be
                               seekable (e.g. this will not work with stdin nor gzipped
                               files).
    --segments                 When used with -c/--chunks, output the byte offsets of
                               found segments instead.
    -f, --filename <filename>  A filename template to use when constructing
                               the names of the output files. The string '{}'
                               will be replaced either by the index in original file of
                               first row emitted when using -S/--size or by the chunk
                               index when using -c/--chunks.
                               [default: {}.csv]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Otherwise, the first row will
                           appear in all chunks as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
