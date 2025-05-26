<!-- Generated -->
# xan count

```txt
Prints a count of the number of records in the CSV data.

Note that the count will not include the header row (unless --no-headers is
given).

You can also use the -p/--parallel or -t/--threads flag to count the number
of records of the file in parallel to go faster. But this cannot work on streams
or gzipped files, unless a `.gzi` index (as created by `bgzip -i`) can be found
beside it.

Usage:
    xan count [options] [<input>]

count options:
    -p, --parallel           Whether to use parallelization to speed up counting.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.
    -a, --approx             Attempt to approximate a CSV file row count by sampling its
                             first rows. Target must be seekable, which means this cannot
                             work on a stream fed through stdin nor with gzipped data.
    --sample-size <n>        Number of rows to sample when using -a, --approx.
                             [default: 512]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
