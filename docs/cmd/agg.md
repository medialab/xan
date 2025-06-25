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

This command can also be used to aggregate a selection of columns per row,
instead of aggregating the whole file, when using the --cols flag. In which
case the expression will take a single variable named `cell`, representing
the value of the column currently processed.

For instance, given the following CSV file:

name,count1,count2
john,3,6
lucy,10,7

Running the following command (notice the `cell` variable in expression):

    $ xan agg --cols count1,count2 'sum(cell) as sum'

Will produce the following output:

name,count1,count2,sum
john,3,6,9
lucy,10,7,17

For a list of available aggregation functions, use `xan help aggs`
instead.

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Aggregations can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the --cols options.

Usage:
    xan agg [options] <expression> [<input>]
    xan agg --help

agg options:
    --cols <columns>         Aggregate a selection of columns per row
                             instead of the whole file. A special `cell`
                             variable will represent the value of a
                             selected column in the aggregation expression.
    -p, --parallel           Whether to use parallelization to speed up computation.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
