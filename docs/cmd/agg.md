<!-- Generated -->
# xan agg

```txt
Aggregate CSV data using a custom aggregation expression. The result of running
the command will be a single row of CSV containing the result of aggregating
the whole file.

You can, for instance, compute the sum of a column:

    $ xan agg 'sum(retweet_count)' file.csv

You can use dynamic expressions to mangle the data before aggregating it:

    $ xan agg 'sum(retweet_count + replies_count)' file.csv

You can perform multiple aggregations at once:

    $ xan agg 'sum(retweet_count), mean(retweet_count), max(replies_count)' file.csv

You can rename the output columns using the 'as' syntax:

    $ xan agg 'sum(n) as sum, max(replies_count) as "Max Replies"' file.csv

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

For a list of available aggregation functions, use the --aggs flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan agg [options] <expression> [<input>]
    xan agg --help
    xan agg --cheatsheet
    xan agg --aggs
    xan agg --functions

agg options:
    -E, --errors <policy>    What to do with evaluation errors. One of:
                               - "panic": exit on first error
                               - "ignore": ignore row altogether
                               - "log": print error to stderr
                             [default: panic].
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores.
    -c, --chunk-size <size>  Number of rows in a batch to send to a thread at once when
                             using -p, --parallel.
                             [default: 4096]

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
