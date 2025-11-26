<!-- Generated -->
# xan bisect

```txt
Search for rows where the value in <column> matches <value> using binary search.
It is assumed that the INPUT IS SORTED according to the specified column.
The ordering of the rows is assumed to be sorted according lexicographic order
per default, but you can specify numeric ordering using the -N or --numeric flag.

Usage:
    xan bisect [options] [--] <column> <value> [<input>]
    xan bisect --help

complete options:
    -N, --numeric            Compare according to the numerical value of cells
                             instead of the default lexicographic order.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
