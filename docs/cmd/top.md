<!-- Generated -->
# xan top

```txt
Find top k values in selected column and return the associated CSV rows.

Runs in O(n * log k) time, n being the number of rows in target CSV file, and
consuming only O(k) memory, which is of course better than piping `xan sort`
into `xan head`.

Note that rows having empty values or values that cannot be parsed as numbers
in selected columns will be ignored along the way.

This command can also return the first k values or last k values in lexicographic
order using the -L/--lexicographic flag (note that the logic of the command is
tailored for numerical values and is therefore the reverse of `xan sort` in this
regard).

Examples:

Top 10 values in "score" column:

    $ xan top score file.csv

Top 50 values:

    $ xan top -l 50 score file.csv

Smallest 10 values:

    $ xan top -R score file.csv

Top 10 values with potential ties:

    $ xan top -T score file.csv

Top 10 values per distinct value of the "category" column:

    $ xan top -g category score file.csv

The same with a preprended "rank" column:

    $ xan top -g category -r rank score file.csv

Last 10 names in lexicographic order:

    $ xan top -L name file.csv

First 10 names in lexicographic order:

    $ xan top -LR name file.csv

Usage:
    xan top <column> [options] [<input>]
    xan top --help

top options:
    -l, --limit <n>          Number of top items to return. Cannot be < 1.
                             [default: 10]
    -R, --reverse            Reverse order.
    -L, --lexicographic      Rank values lexicographically instead of considering
                             them as numbers.
    -g, --groupby <cols>     Return top n values per group, represented
                             by the values in given columns.
    -r, --rank <col>         Name of a rank column to prepend.
    -T, --ties               Keep all rows tied for last. Will therefore
                             consume O(k + t) memory, t being the number of ties.
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
