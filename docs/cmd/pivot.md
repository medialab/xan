<!-- Generated -->
# xan pivot

```txt
Pivot a CSV file by allowing distinct values from a column to be separated into
their own column.

For instance, given the following data:

country,name,year,population
NL,Amsterdam,2000,1005
NL,Amsterdam,2010,1065
NL,Amsterdam,2020,1158
US,Seattle,2000,564
US,Seattle,2010,608
US,Seattle,2020,738
US,New York City,2000,8015
US,New York City,2010,8175
US,New York City,2020,8772

The following command:

    $ xan pivot year 'first(population)' file.csv

Will produce the following result:

country,name,2000,2010,2020
NL,Amsterdam,1005,1065,1158
US,Seattle,564,608,738
US,New York City,8015,8175,8772

By default, rows will be grouped and aggregated together using all columns that
are not the pivoted column nor present in the aggregation clause. If you want
to group rows differently, you can use the -g/--groupby flag instead so that
the following command:

    $ xan pivot year 'sum(population)' -g country file.csv

Will produce:

country,2000,2010,2020
NL,1005,1065,1158
US,564,608,738

The command can also be called without <column> nor <expr> as a convenient
shorthand where they will stand for "name" and "first(value)" respectively so
you can easily call `xan pivot` downstream of `xan unpivot`:

    $ xan unpivot january: monthly.csv | <processing> | xan pivot

Usage:
    xan pivot [-P...] [options] <columns> <expr> [<input>]
    xan pivot [-P...] [options] [<input>]
    xan pivot --help

pivot options:
    -g, --groupby <columns>  Group results by given selection of columns instead
                             of grouping by columns not used to pivot nor in
                             aggregation.
    --column-sep <sep>       Separator used to join column names when pivoting
                             on multiple columns. [default: _]

pivotal options:
    -P  Use at least three times to get help from your friends!

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
