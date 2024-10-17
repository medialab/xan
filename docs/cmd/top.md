<!-- Generated -->
# xan top

```txt
Find top k CSV rows according to some column values.

Runs in O(N * log k) time, consuming only O(k) memory.

Usage:
    xan top <column> [options] [<input>]
    xan top --help

dedup options:
    -l, --limit <n>       Number of top items to return. Cannot be < 1.
                          [default: 10]
    -R, --reverse         Reverse order.
    -g, --groupby <cols>  Return top n values per group, represented
                          by the values in given columns.
    -r, --rank <col>      Name of a rank column to prepend.
    -T, --ties            Keep all rows tied for last. Will therefore
                          consume O(k + t) memory, t being the number of ties.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
