<!-- Generated -->
# xan shuffle

```txt
Shuffle the rows of a given CSV file. This requires loading the whole
file in memory. If memory is scarce and target file is seekable (not stdin,
nor unindexed compressed file), you can also use the -e/--external flag that
only requires memory proportional to the number of rows of the file.

Usage:
    xan shuffle [options] [<input>]
    xan shuffle --help

shuffle options:
    --seed <number>  RNG seed.
    -e, --external   Shuffle the file without buffering it into memory. Only
                     works if target is seekable (no stdin etc.).

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
