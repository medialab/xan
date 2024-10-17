<!-- Generated -->
# xan frequency

```txt
Compute a frequency table on CSV data.

The resulting frequency table will look like this:

field - Name of the column
value - Some distinct value of the column
count - Number of rows containing this value

By default, there is a row for the N most frequent values for each field in the
data. The number of values can be tweaked with --limit and --threshold flags
respectively.

Since this computes an exact frequency table, memory proportional to the
cardinality of each selected column is required.

To compute custom aggregations per group, beyond just counting, please be sure to
check the `xan groupby` command instead.

Usage:
    xan frequency [options] [<input>]
    xan freq [options] [<input>]

frequency options:
    -s, --select <arg>     Select a subset of columns to compute frequencies
                           for. See 'xan select --help' for the format
                           details. This is provided here because piping 'xan
                           select' into 'xan frequency' will disable the use
                           of indexing.
    --sep <char>           Split the cell into multiple values to count using the
                           provided separator.
    -g, --groupby <cols>   If given, will compute frequency tables per group
                           as defined by the given columns.
    -l, --limit <arg>      Limit the frequency table to the N most common
                           items. Set to <=0 to disable a limit. It is combined
                           with -t/--threshold.
                           [default: 10]
    -t, --threshold <arg>  If set, won't return items having a count less than
                           this given threshold. It is combined with -l/--limit.
    -N, --no-extra         Don't include empty cells & remaining counts.
    -p, --parallel         Allow sorting to be done in parallel. This is only
                           useful with -l/--limit set to 0, i.e. no limit.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 1-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
