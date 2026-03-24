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

Note that the difference between the `adj` and `count` mode is that `count`
considers its `x` & `y` labels as two separate sets while `adj` considers `source`
and `target` labels as parts of the same set. This also means `adj` produces a
square matrix while `count` produces a rectangular one.

Usage:
    xan matrix adj [options] <source> <target> [<input>]
    xan matrix count [options] <x> <y> [<input>]
    xan matrix corr [options] [<input>]
    xan matrix --help

matrix adj/count options:
    -w, --weight <column>  Optional column containing a weight for edges.

matrix corr options:
    -s, --select <columns>  Columns to consider for the correlation
                            matrix.
    -D, --fill-diagonal     Whether to fill diagonal with ones.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
```
