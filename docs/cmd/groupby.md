<!-- Generated -->
# xan groupby

```txt
Group a CSV file by values contained in a column selection then aggregate data per
group using a custom aggregation expression.

The result of running the command will be a CSV file containing the grouped
columns and additional columns for each computed aggregation.

You can, for instance, compute the sum of a column per group:

    $ xan groupby user_name 'sum(retweet_count)' file.csv

You can use dynamic expressions to mangle the data before aggregating it:

    $ xan groupby user_name 'sum(retweet_count + replies_count)' file.csv

You can perform multiple aggregations at once:

    $ xan groupby user_name 'sum(retweet_count), mean(retweet_count), max(replies_count)' file.csv

You can rename the output columns using the 'as' syntax:

    $ xan groupby user_name 'sum(n) as sum, max(replies_count) as "Max Replies"' file.csv

You can group on multiple columns (read `xan select -h` for more information about column selection):

    $ xan groupby name,surname 'sum(count)' file.csv

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

For a list of available aggregation functions, use the --aggs flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan groupby [options] <column> <expression> [<input>]
    xan groupby --help
    xan groupby --cheatsheet
    xan groupby --aggs
    xan groupby --functions

groupby options:
    -S, --sorted            Use this flag to indicate that the file is already sorted on the
                            group columns, in which case the command will be able to considerably
                            optimize memory usage.
    -e, --errors <policy>   What to do with evaluation errors. One of:
                              - "panic": exit on first error
                              - "ignore": ignore row altogether
                              - "log": print error to stderr
                            [default: panic].
    -p, --parallel          Whether to use parallelization to speed up computations.
                            Will automatically select a suitable number of threads to use
                            based on your number of cores.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```