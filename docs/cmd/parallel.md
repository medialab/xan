<!-- Generated -->
# xan parallel

```txt
Parallel processing of either a single, uncompressed CSV file that
will be read as chunks or a dataset of multiple files.

To process a single CSV file in parallel, use the -F/--single-file flag:

    $ xan parallel count -F docs.csv

To process multiple files at once, you must give their paths as multiple
arguments to the command or give them through stdin with one path
per line or in a CSV column when using the --path-column flag:

    Multiple arguments through shell glob:
    $ xan parallel count data/**/docs.csv

    One path per line, fed through stdin:
    $ ls data/**/docs.csv | xan parallel count

    Paths from a CSV column through stdin:
    $ cat filelist.csv | xan parallel count --path-column path

Note that you can use the `split` or `partition` command to preemptively
split a large file into manageable chunks, if you can spare the disk space.

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
    xan p count [options] [<inputs>...]
    xan p cat [options] [<inputs>...]
    xan p freq [options] [<inputs>...]
    xan p stats [options] [<inputs>...]
    xan p agg [options] <expr> [<inputs>...]
    xan p groupby [options] <group> <expr> [<inputs>...]
    xan parallel --help
    xan p --help

parallel options:
    -F, --single-file            Parallelize computation over a single uncompressed
                                 CSV file on disk instead of processing multiple
                                 files in parallel.
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

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
