<!-- Generated -->
# xan agg

```txt
Aggregate CSV data using custom aggregation expressions.

For typical statistics, check out the `xan stats` command that is usually
simpler to use.

For grouped aggregation, check out the `xan groupby` command instead.

# Custom aggregation

When running a custom aggregation, the result will be a single row of CSV
containing the result of aggregating the whole file.

For instance, given the following CSV file:

| name | count1 | count2 |
| ---- | ------ | ------ |
| john | 3      | 6      |
| lucy | 10     | 7      |

Running the following command:

    $ xan agg 'sum(count1) as sum1, sum(count2) as sum2'

Will produce the following output:

| sum1 | sum2 |
| ---- | ---- |
| 13   | 13   |

Check out the following example to learn how to compose your expressions. Note
that a complete list of aggregation functions can be found using `xan help aggs`.

Computing the sum of a column:

    $ xan agg 'sum(retweet_count)' file.csv

Using dynamic expressions to mangle the data before aggregation:

    $ xan agg 'sum(retweet_count + replies_count)' file.csv

Multiple aggregations at once:

    $ xan agg 'sum(retweet_count), mean(retweet_count), max(replies_count)' file.csv

Renaming the output columns using the 'as' syntax:

    $ xan agg 'sum(n) as sum, max(replies_count) as "Max Replies"' file.csv

# Aggregating along rows

This command can be used to aggregate a selection of columns per row,
instead of aggregating the whole file, when using the --along-rows flag. In
which case aggregation functions will accept the anonymous `_` placeholder value
representing the currently processed column's value.

In a way, it is a variant of `xan map`, able to leverage aggregation
functions and generic over target columns.

Note that when using --along-rows, the `index()` function will return the
index of currently processed column, not the row index. This can be useful
when used with `argmin/argmax` etc.

For instance, given the following CSV file:

| name | count1 | count2 |
| ---- | ------ | ------ |
| john | 3      | 6      |
| lucy | 10     | 7      |

Running the following command (notice the `_` in expression):

    $ xan agg --along-rows count1,count2 'sum(_) as sum'

Will produce the following output:

| name | count1 | count2 | sum |
| ---- | ------ | ------ | --- |
| john | 3      | 6      | 9   |
| lucy | 10     | 7      | 17  |

Typical use-cases include getting the variance of the dimensions of
dense vectors:

    $ xan agg -R 'dim_*' 'var(_) as variance' vectors.csv

Finding the column maximizing a score:

    $ xan agg -R '*_score' 'argmax(_, header(index()) as best' results.csv

# Aggregating along columns

This command can also be used to run a same aggregation over a selection of commands
using the -C/--along-columns flag. In which case aggregation functions will accept
the anonymous `_` placeholder value representing the currently processed column's value.

For instance, given the following file:

| name | count1 | count2 |
| ---- | ------ | ------ |
| john | 3      | 6      |
| lucy | 10     | 7      |

Running the following command (notice the `_` in expression):

    $ xan agg --along-cols count1,count2 'sum(_)'

Will produce the following output:

| count1 | count2 |
| ------ | ------ |
| 13     | 13     |

# Aggregating along matrix

This command can also be used to run a custom aggregation over all values of
a selection of columns thus representing a 2-dimensional matrix, using
the -M/--along-matrix flag. In which case aggregation functions will accept
the anonymous `_` placeholder value representing the currently processed column's value.

For instance, given the following file:

| name | count1 | count2 |
| ---- | ------ | ------ |
| john | 3      | 6      |
| lucy | 10     | 7      |

Running the following command (notice the `_` in expression):

    $ xan agg --along-matrix count1,count2 'sum(_) as total'

Will produce the following output:

| total |
| ----- |
| 26    |

---

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available aggregation functions use `xan help aggs`.

For a list of available functions, use `xan help functions`.

Aggregations can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -R/--along-rows, -M/--along-matrix nor -C/--along-cols options.

Usage:
    xan agg [options] <expression> [<input>]
    xan agg --help

agg options:
    -R, --along-rows <cols>    Aggregate a selection of columns for each row
                               instead of the whole file.
    -C, --along-cols <cols>    Aggregate a selection of columns the same way and
                               return an aggregated column with same name in the
                               output.
    -M, --along-matrix <cols>  Aggregate all values found in the given selection
                               of columns.
    -p, --parallel             Whether to use parallelization to speed up computation.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
