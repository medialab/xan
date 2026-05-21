<!-- Generated -->
# xan stats

```txt
Computes descriptive statistics of CSV data.

If you want to print human-readable output, use the -R/--report flag.

Else this command can be used to generate a CSV output that can be easily piped
into other `xan` commands.

By default, statistics are reported for *every* column in the CSV data, but you
can restrict the set of analyzed columns using the -s/--select flag.

The default set of statistics corresponds to things that can be computed efficiently
on a stream in constant memory, but more can be selected using flags documented
hereafter.

Stats can also be computed per group using the -g/--groupby flag.

If you have more specific needs or want to perform custom aggregations, please be
sure to check the `xan agg` or `xan groupby` commands instead.

Here is what the CSV output will look like:

field              (default) - Name of the described column
count              (default) - Number of non-empty values contained by the column
count_empty        (default) - Number of empty values contained by the column
type               (default) - Most likely type of the column
types              (default) - Pipe-separated list of all types witnessed in the column
sum                (default) - Sum of numerical values
mean               (default) - Mean of numerical values
q1                 (-q, -A)  - First quartile of numerical values
median             (-q, -A)  - Second quartile, i.e. median, of numerical values
q3                 (-q, -A)  - Third quartile of numerical values
log_dist           (-q, -A)  - Sparkline (e.g. ▇▅▄▃▂▃▂▂▂▂) representing numerical distribution
variance           (default) - Population variance of numerical values
stddev             (default) - Population standard deviation of numerical values
min                (default) - Minimum numerical value
max                (default) - Maximum numerical value
approx_cardinality (-a)      - Approximation of the number of distinct string values
approx_q1          (-a)      - Approximation of the first quartile of numerical values
approx_median      (-a)      - Approximation of the median of numerical values
approx_q3          (-a)      - Approximation of the third quartile of numerical values
cardinality        (-c, -A)  - Number of distinct string values
mode               (-c, -A)  - Most frequent string value (tie breaking is arbitrary & random!)
tied_for_mode      (-c, -A)  - Number of values tied for mode
lex_first          (default) - First string in lexical order
lex_last           (default) - Last string in lexical order
min_length         (default) - Minimum string length
max_length         (default) - Maximum string length

Stats can be computed in parallel using the -p/--parallel or -t/--threads flags.
But this cannot work on streams or gzipped files, unless a `.gzi` index (as created
by `bgzip -i`) can be found beside it. Parallelization is not compatible
with the -g/--groupby option.

Note that the output of the -R/--report can easily be piped into a pager
thusly (don't forget to force colors):

    $ xan stats -R data.csv --color always | less -SR

Usage:
    xan stats [options] [<input>]

stats options:
    -s, --select <arg>       Select a subset of columns to compute stats for.
                             See 'xan select --help' for the format details.
                             This is provided here because piping 'xan select'
                             into 'xan stats' will disable the use of indexing.
    -R, --report             Print a human-readable output suitable to understand
                             what your columns contain, along with the relevant
                             data visualizations (bar charts, top lists etc.)
                             Does not work with -g/--groupby.
    --sep <str>              Indicate that cells must be split using given separator.
    -g, --groupby <cols>     If given, will compute stats per group as defined by
                             the given column selection.
    -A, --all                Shorthand for -cq.
    -c, --cardinality        Show cardinality and modes.
                             This requires storing all CSV data in memory.
    -q, --quartiles          Show quartiles.
                             This requires storing all CSV data in memory.
    -a, --approx             Compute approximated statistics.
    --nulls                  Include empty values in the population size for computing
                             mean and standard deviation.
    -p, --parallel           Whether to use parallelization to speed up computation.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

stats -R/--report options:
    --cols <num>    Width of the graph in terminal columns, i.e. characters.
                    Defaults to using all your terminal's width or 80 if
                    terminal's size cannot be found (i.e. when piping to file).
                    Can also be given as a ratio or percentage of the terminal's width
                    e.g. "45%" or "0.5".
    --color <when>  When to color the output using ANSI escape codes.
                    Use `auto` for automatic detection, `never` to
                    disable colors completely and `always` to force
                    colors, even when the output could not handle them.
                    [default: auto]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. i.e., They will be included
                           in statistics.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
