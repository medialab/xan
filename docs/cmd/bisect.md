<!-- Generated -->
# xan bisect

```txt
Search for rows where the value in <column> matches <value> using binary search,
and flush all records after the target value.
The default behavior is similar to a lower_bound bisection, but you can exclude
records (equivalent to upper_bound) with the target value using the -E/--exclude
flag. It is assumed that the INPUT IS SORTED according to the specified column.
The ordering of the rows is assumed to be sorted according ascending lexicographic
order per default, but you can specify numeric ordering using the -N or --numeric
flag. You can also reverse the order using the -R/--reverse flag.
Use the -S/--search flag to only flush records matching the target value instead
of all records after it.

Usage:
    xan bisect [options] [--] <column> <value> <input>
    xan bisect --help

bisect options:
    -E, --exclude            When set, the records with the target value will be
                             excluded from the output. By default, they are
                             included. Cannot be used with -S/--search.
                             TODO: not equivalent to upper_bound
    -N, --numeric            Compare according to the numerical value of cells
                             instead of the default lexicographic order.
    -R, --reverse            Reverse sort order, i.e. descending order.
    -S, --search             Perform a search on the target value instead of
                             flushing all records after the value (included).
                             Cannot be used with -E/--exclude nor -e/--end.
    -e, --end <end-value>    When set, the records after the target value will be
                             flushed until <end-value> is reached (included).
                             By default, all records after the target value are
                             flushed. Cannot be used with -S/--search.
    -v, --verbose

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
