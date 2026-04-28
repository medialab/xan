<!-- Generated -->
# xan window

```txt
Compute window aggregations such as cumulative sums, rolling means, leading and
lagging values, rankings etc.

This command is able to compute multiple aggregations in a single pass over the
file, and never uses more memory that required to fit the largest desired window
for rolling stats and leads/lags.

Ranking aggregations however (such as `frac` or `dense_rank`), still require to
buffer the whole file in memory.

Aggregations can also be computed per group using the -g/--groupby <cols> flag
but will also require to buffer the whole file in memory, unless you can
guarantee the file is sorted on the grouping columns and use the -S/--sorted flag
to indicate it to the command.

Computing a cumulative sum:

    $ xan window 'cumsum(n)' file.csv

Computing a rolling mean & variance:

    $ xan window 'rolling_mean(10, n) as mean, rolling_var(10, n) as var' file.csv

Adding a lagged column:

    $ xan window 'lag(n) as "n-1"' file.csv

Ranking numerical values:

    $ xan window 'dense_rank(n) as rank' file.csv

Computing fraction of cell wrt total sum of target column:

    $ xan window 'frac(n) as frac' file.csv

Computing window aggregations per value of the "country" column:

    $ xan window -g country 'cumsum(n)' file.csv

Finally, this command can also run arbitrary aggregation functions (like
with `xan agg` & `xan groupby`) for the whole file or per group and repeat their
result for each row. This can be useful to filter rows belonging to some group
(e.g. if an aggregated score is over some threshold), or for normalization purposes.

Note that when doing so, the whole file (or only whole groups when using -g/--groupby
alongside -S/--sorted) will be buffered to memory.

Keeping rows belonging to groups whose average for the `count` column is over 10:

    $ xan window -g country 'mean(count) as mean' file.csv | xan filter 'mean > 10'

# Window aggregationgs along columns

Sometimes you might want to add one or more columns in a same fashion for a given
selection of columns.

You can do so using the -C/--along-columns <cols> flag. In this case, the `_`
placeholder can be used in expression to represent the current column.

For instance, given the following data:

a,b
4,5
1,7

The following command (notice how we can template added column names):

    $ xan window -C a,b 'mean(_) as "{}_mean", lag(_) as "{}_lag"' file.csv

Would produce the following:

a,a_mean,a_lag,b,b_mean,b_lag
4,2.5,,5,6.0,
1,2.5,4,7,6.0,5

This can also be used with the -O/--overwrite flag:

    $ xan window -OC a,b 'mean(_) as "{}_mean", lag(_) as "{}_lag"' file.csv

To produce:

a_mean,a_lag,b_mean,b_lag
2.5,,6.0,
2.5,4,6.0,5

---

For a list of available window aggregation functions, use `xan help window`.

For a list of available generic aggregation functions, use `xan help aggs`.

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan window [options] <expression> [<input>]
    xan window --help

window options:
    -g, --groupby <cols>        If given, runs the aggregation per group symbolized by
                                given column selection. This will buffer the whole file
                                into memory unless -S/--sorted is given.
    -S, --sorted                When used with -g/--groupby, indicates that the file is
                                sorted over the group columns so we can reset state each
                                time a new group is encountered to save memory and speed
                                up computations.
    -O, --overwrite             If set, expressions named with a column already existing
                                in the file will be overwritten with the result of the
                                expression instead of adding a new column at the end.
                                This means you can both transform and add columns at the
                                same time.
    -C, --along-columns <cols>  Repeat same expression over a selection of columns at once.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
