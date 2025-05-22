<!-- Generated -->
# xan frequency

```txt
Compute a frequency table on CSV data.

The resulting frequency table will look like this:

field - Name of the column
value - Some distinct value of the column
count - Number of rows containing this value

By default, there is a row for the N most frequent values for each field in the
data. The number of returned values can be tweaked with -l/--limit or you can
disable the limit altogether using the -A/--all flag.

Since this computes an exact frequency table, memory proportional to the
cardinality of each selected column is required. If you expect this will overflow
your memory, you can compute an approximate top-k using the -a, --approx flag.

To compute custom aggregations per group, beyond just counting, please be sure to
check the `xan groupby` command instead.

Usage:
    xan frequency [options] [<input>]
    xan freq [options] [<input>]

frequency options:
    -s, --select <arg>     Select a subset of columns to compute frequencies
                           for. See 'xan select --help' for the selection language
                           details.
    --sep <char>           Split the cell into multiple values to count using the
                           provided separator.
    -g, --groupby <cols>   If given, will compute frequency tables per group
                           as defined by the given columns.
    -A, --all              Remove the limit.
    -l, --limit <arg>      Limit the frequency table to the N most common
                           items. Use -A, -all or set to 0 to disable the limit.
                           [default: 10]
    -a, --approx           If set, return the items most likely having the top counts,
                           as per given --limit. Won't work if --limit is 0 or
                           with -A, --all. Accuracy of results increases with the given
                           limit.
    -N, --no-extra         Don't include empty cells & remaining counts.
    -p, --parallel         Allow sorting to be done in parallel. This is only
                           useful with -l/--limit set to 0, or with -A, --all.

Hidden options:
    --no-limit-we-reach-for-the-sky  Nothing to see here...

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 0-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
