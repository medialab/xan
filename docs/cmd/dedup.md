<!-- Generated -->
# xan dedup

```txt
Deduplicate the rows of a CSV file. Runs in O(n) time, consuming O(c) memory, c being
the distinct number of row identities.

If your file is already sorted on the deduplication selection, use the -S/--sorted flag
to run in O(1) memory instead.

Note that it will be the first row having a specific identity that will be emitted in
the output and not any subsequent one.

Usage:
    xan dedup [options] [<input>]
    xan dedup --help

dedup options:
    -s, --select <arg>  Select a subset of columns to on which to deduplicate.
                        See 'xan select --help' for the format details.
    -S, --sorted        Use if you know your file is already sorted on the deduplication
                        selection to avoid storing unique values in memory.
    -l, --keep-last     Keep the last row having a specific identiy, rather than
                        the first one. Note that it will cost more memory and that
                        no rows will be flushed before the whole file has been read
                        if -S/--sorted is not used.
    -e, --external      Use an external btree index to keep the index on disk and avoid
                        overflowing RAM. Does not work with -l/--keep-last.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
