<!-- Generated -->
# xan groupby

```txt
Group a CSV file by values contained in a column selection then aggregate data per
group using a custom aggregation expression.

For ungrouped aggregation, check the `xan agg` command instead.

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

# Aggregating along columns

This command is also able to aggregate along columns that you can select using
the --along-cols <cols> flag. In which case, the aggregation functions will accept
the anonymous `_` placeholder representing currently processed column's value.

For instance, given the following file:

user,count1,count2
marcy,4,5
john,0,1
marcy,6,8
john,4,6

Using the following command:

    $ xan groupby user --along-cols count1,count2 'sum(_)' file.csv

Will produce the following result:

user,count1,count2
marcy,10,13
john,4,7

# Aggregating along matrix

This command can also aggregate over all values of a selection of columns, thus
representing a 2-dimensional matrix, using the -M/--along-matrix flag. In which
case aggregation functions will accept the anonymous `_` placeholder value representing
the currently processed column's value.

For instance, given the following file:

user,count1,count2
marcy,4,5
john,0,1
marcy,6,8
john,4,6

Using the following command:

    $ xan groupby user --along-matrix count1,count2 'sum(_) as total' file.csv

Will produce the following result:

user,total
marcy,23
john,11

---

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available aggregation functions, use `xan help aggs`.

For a list of available functions, use `xan help functions`.

Aggregations can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -S/--sorted nor -C/--along-cols flags.

Usage:
    xan groupby [options] <column> <expression> [<input>]
    xan groupby --help

groupby options:
    --keep <cols>              Keep this selection of columns, in addition to
                               the ones representing groups, in the output. Only
                               values from the first seen row per group will be kept.
    -C, --along-cols <cols>    Perform a single aggregation over all of selected columns
                               and create a column per group with the result in the output.
    -M, --along-matrix <cols>  Aggregate all values found in the given selection
                               of columns.
    -S, --sorted               Use this flag to indicate that the file is already sorted on the
                               group columns, in which case the command will be able to considerably
                               optimize memory usage.
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
