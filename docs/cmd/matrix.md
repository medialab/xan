<!-- Generated -->
# xan matrix

```txt
Convert CSV data to matrix data.

Supported modes:
    adj   - convert a column of sources & a column of targets into
            an adjacency matrix.
    count - convert a pair of columns into a full count matrix (a bipartite
            adjacency matrix, or co-occurrence matrix, if you will).
    corr  - convert a selection of columns into a full
            correlation matrix.
    bivar - convert x & y columns into a bivariate distribution matrix (e.g. a
            discretized scatterplot).

Note that the difference between the `adj` and `count` mode is that `count`
considers its `x` & `y` labels as two separate sets while `adj` considers `source`
and `target` labels as parts of the same set. This also means `adj` produces a
square matrix while `count` produces a rectangular one.

Usage:
    xan matrix adj [options] <source> <target> [<input>]
    xan matrix count [options] <x> <y> [<input>]
    xan matrix corr [options] [<input>]
    xan matrix bivar [options] <x> <y> [<input>]
    xan matrix --help

matrix adj/count/bivar options:
    -w, --weight <column>  Optional column containing numbers that will be used
                           as matrix cell weights, instead of just counting
                           occurrences.

matrix adj options:
    -U, --undirected  Indicates that edges are undirected and that produced
                      matrix should be symmetric.

matrix corr options:
    -s, --select <columns>  Columns to consider for the correlation
                            matrix.
    -D, --fill-diagonal     Whether to fill diagonal with ones.

matrix bivar options:
    -b, --bins <nb_columns>  Number of columns (= number of rows) to consider
                             for the square matrix [default: 10]
    --x-bins <nb_columns>    Number of columns to consider for the matrix
                             Default to --bins.
    --y-bins <nb_rows>       Number of rows to consider for the matrix
                             Default to --bins.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
```
