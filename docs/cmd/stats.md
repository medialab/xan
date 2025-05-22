<!-- Generated -->
# xan stats

```txt
Computes descriptive statistics on CSV data.

By default, statistics are reported for *every* column in the CSV data. The default
set of statistics corresponds to statistics that can be computed efficiently on a
stream of data in constant memory, but more can be selected using flags documented
hereafter.

If you have more specific needs or want to perform custom aggregations, please be
sure to check the `xan agg` command instead.

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
But note that this cannot work on streams or gzipped data and does not support
the -g/--groubpy flag.

Usage:
    xan stats [options] [<input>]

stats options:
    -s, --select <arg>       Select a subset of columns to compute stats for.
                             See 'xan select --help' for the format details.
                             This is provided here because piping 'xan select'
                             into 'xan stats' will disable the use of indexing.
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
    -p, --parallel           Whether to use parallelization to speed up counting.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. i.e., They will be included
                           in statistics.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
