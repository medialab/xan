<!-- Generated -->
# xan parallel

```txt
Process CSV datasets split into multiple files, in parallel.

The CSV files composing said dataset can be given as variadic arguments to the
command, or given through stdin, one path per line or in a CSV column when
using --path-column.

`xan parallel count` counts the number of rows in the whole dataset.

`xan parallel cat` preprocess the files and redirect the concatenated
rows to your output (e.g. searching all the files in parallel and
retrieving the results).

`xan parallel freq` build frequency tables in parallel.

Note that you can use the `split` or `partition` command to preemptively
split a large file into manageable chunks, if you can spare the disk space.

Usage:
    xan parallel count [options] [<inputs>...]
    xan parallel cat [options] [<inputs>...]
    xan parallel freq [options] [<inputs>...]
    xan p count [options] [<inputs>...]
    xan p cat [options] [<inputs>...]
    xan p freq [options] [<inputs>...]
    xan parallel --help

parallel options:
    -P, --preprocess <op>  Preprocessing command that will run on every
                           file to process.
    --progress             Display a progress bar for the parallel tasks.
    -t, --threads <n>      Number of threads to use. Will default to a sensible
                           number based on the available CPUs.
    --path-column <name>   Name of the path column if stdin is given as a CSV file
                           instead of one path per line.

parallel cat options:
    -B, --buffer-size <n>  Number of rows a thread is allowed to keep in memory
                           before flushing to the output.
                           [default: 1024]

parallel freq options:
    -s, --select <cols>  Columns for which to build frequency tables.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
