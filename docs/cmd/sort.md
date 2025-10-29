<!-- Generated -->
# xan sort

```txt
Sort CSV data, in ascending lexicographic order.

For descending order, use the -R, --reverse flag.

If you need numerical order instead, use the -N, --numeric flag.

This requires reading all of the data into memory, unless
using the -e/--external flag, which will be slower and fallback
to using disk space.

Usage:
    xan sort [options] [<input>]

sort options:
    --check                   Verify whether the file is already sorted.
    -s, --select <arg>        Select a subset of columns to sort by.
                              See 'xan select --help' for the format details.
    -N, --numeric             Compare according to the numerical value of cells instead
                              of the default lexicographic order.
    -R, --reverse             Reverse sort order, i.e. descending order.
    -c, --count <name>        Number of times the line was consecutively duplicated.
                              Needs a column name. Can only be used with --uniq.
    -u, --uniq                When set, identical consecutive lines will be dropped
                              to keep only one line per sorted value.
    -U, --unstable            Unstable sort. Can improve performance. Does not work
                              with -e/--external.
    -p, --parallel            Whether to use parallelism to improve performance.
    -e, --external            Whether to use external sorting if you cannot fit the
                              whole file in memory.
    --tmp-dir <arg>           Directory where external sorting chunks will be written.
                              Will default to the sorted file's directory or "./" if
                              sorting an incoming stream.
    -m, --memory-limit <arg>  Maximum allowed memory when using external sorting, in
                              megabytes. [default: 512].
    -C, --cells               Sort the selected cell values instead of the file itself,
                              without re-ordering the columns. Runs in constant memory,
                              can be streamed and can be used to e.g. make sure an
                              edgelist always has the source & target keys in a consistent
                              order.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Namely, it will be sorted with the rest
                           of the rows. Otherwise, the first row will always
                           appear as the header row in the output.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
