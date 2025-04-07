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

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

For a list of available aggregation functions, use `xan help aggs`
instead.

Usage:
    xan agg [options] <expression> [<input>]
    xan agg --help

agg options:
    -E, --errors <policy>    What to do with evaluation errors. One of:
                               - "panic": exit on first error
                               - "ignore": ignore row altogether
                               - "log": print error to stderr
                             [default: panic].
    --cols <columns>         Aggregate a selection of columns per row
                             instead of the whole file. A special `cell`
                             variable will represent the value of a
                             selected column in the aggregation expression.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
