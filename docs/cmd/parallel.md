<!-- Generated -->
# xan parallel

```txt
Parallel processing of CSV data.

This command usually parallelizes computation over multiple files, but is also
able to automatically chunk CSV files and bgzipped CSV files (when a `.gzi` index
can be found) when the number of available threads is greater than the number
of files to read.

This means this command is quite capable of parallelizing over a single CSV file.

To process a single CSV file in parallel:

    $ xan parallel count docs.csv

To process multiple files at once, you must give their paths as multiple
arguments to the command or give them through stdin with one path
per line or in a CSV column when using the --path-column flag:

    Multiple arguments through shell glob:
    $ xan parallel count data/**/docs.csv

    One path per line, fed through stdin:
    $ ls data/**/docs.csv | xan parallel count

    Paths from a CSV column through stdin:
    $ cat filelist.csv | xan parallel count --path-column path

Note that sometimes you might find useful to use the `split` or `partition`
command to preemptively split a large file into manageable chunks, if you can
spare the disk space.

This command has multiple subcommands that each perform some typical
parallel reduce operation:

    - `count`: counts the number of rows in the whole dataset.
    - `cat`: preprocess the files and redirect the concatenated
        rows to your output (e.g. searching all the files in parallel and
        retrieving the results).
    - `freq`: builds frequency tables in parallel. See "xan freq -h" for
        an example of output.
    - `stats`: computes well-known statistics in parallel. See "xan stats -h" for
        an example of output.
    - `agg`: parallelize a custom aggregation. See "xan agg -h" for more details.
    - `groupby`: parallelize a custom grouped aggregation. See "xan groupby -h"
        for more details.
    - `map`: writes the result of given preprocessing in a new
        file besides the original one. This subcommand takes a filename template
        where `{}` will be replaced by the name of each target file without any
        extension (`.csv` or `.csv.gz` would be stripped for instance). This
        command is unable to leverage CSV file chunking.

For instance, the following command:

    $ xan parallel map '{}_freq.csv' -P 'freq -s Category' *.csv

Will create a file suffixed "_freq.csv" for each CSV file in current directory
containing its frequency table for the "Category" command.

Finally, preprocessing on each file can be done using two different methods:

1. Using only xan subcommands with -P, --preprocess:
    $ xan parallel count -P "search -s name John | slice -l 10" file.csv

2. Using a shell subcommand passed to "$SHELL -c" with -H, --shell-preprocess:
    $ xan parallel count -H "xan search -s name John | xan slice -l 10" file.csv

The second preprocessing option will of course not work in DOS-based shells and Powershell
on Windows.

Usage:
    xan parallel count [options] [<inputs>...]
    xan parallel cat [options] [<inputs>...]
    xan parallel freq [options] [<inputs>...]
    xan parallel stats [options] [<inputs>...]
    xan parallel agg [options] <expr> [<inputs>...]
    xan parallel groupby [options] <group> <expr> [<inputs>...]
    xan parallel map <template> [options] [<inputs>...]
    xan parallel --help
    xan p count [options] [<inputs>...]
    xan p cat [options] [<inputs>...]
    xan p freq [options] [<inputs>...]
    xan p stats [options] [<inputs>...]
    xan p agg [options] <expr> [<inputs>...]
    xan p groupby [options] <group> <expr> [<inputs>...]
    xan p map <template> [options] [<inputs>...]
    xan p --help

parallel options:
    -P, --preprocess <op>        Preprocessing, only able to use xan subcommands.
    -H, --shell-preprocess <op>  Preprocessing commands that will run directly in your
                                 own shell using the -c flag. Will not work on windows.
    --progress                   Display a progress bar for the parallel tasks. The
                                 per file/chunk bars will tick once per CSV row only
                                 AFTER pre-processing!
    -t, --threads <n>            Number of threads to use. Will default to a sensible
                                 number based on the available CPUs.
    --path-column <name>         Name of the path column if stdin is given as a CSV file
                                 instead of one path per line.

parallel count options:
    -S, --source-column <name>  If given, will return a CSV file containing a column with
                                the source file being counted and a column with the count itself.

parallel cat options:
    -B, --buffer-size <n>       Number of rows a thread is allowed to keep in memory
                                before flushing to the output. Set <= 0 to flush only once per
                                processed file. Keep in mind this could cost a lot of memory.
                                [default: 1024]
    -S, --source-column <name>  Name of a column to prepend in the output of indicating the
                                path to source file.

parallel freq options:
    -s, --select <cols>  Columns for which to build frequency tables.
    --sep <char>         Split the cell into multiple values to count using the
                         provided separator.
    -A, --all            Remove the limit.
    -l, --limit <arg>    Limit the frequency table to the N most common
                         items. Use -A, -all or set to 0 to disable the limit.
                         [default: 10]
    -a, --approx         If set, return the items most likely having the top counts,
                         as per given --limit. Won't work if --limit is 0 or
                         with -A, --all. Accuracy of results increases with the given
                         limit.
    -N, --no-extra       Don't include empty cells & remaining counts.

parallel stats options:
    -s, --select <cols>    Columns for which to build statistics.
    -A, --all              Shorthand for -cq.
    -c, --cardinality      Show cardinality and modes.
                           This requires storing all CSV data in memory.
    -q, --quartiles        Show quartiles.
                           This requires storing all CSV data in memory.
    -a, --approx           Show approximated statistics.
    --nulls                Include empty values in the population size for computing
                           mean and standard deviation.

parallel map options:
    -z, --compress  Use this flag to gzip the processed files.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
