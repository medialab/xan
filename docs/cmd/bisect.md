<!-- Generated -->
# xan bisect

```txt
Perform binary search on sorted CSV data.

This command is one order of magnitude faster than relying on `xan filter` or
`xan search` but only works if target file is sorted on searched column, exists
on disk and is not compressed (unless the compressed file remains seekable,
typically if some `.gzi` index can be found beside it).

If CSV data is not properly sorted, result will be incorrect!

By default this command executes the so-called "lower bound" operation: it
positions itself in the file where one would insert the searched value and then
proceeds to flush the file from this point. This can be useful when piping
into other commands to perform range queries, for instance, or enumerate values
starting with some prefix.

Use the -S/--search flag if you only want to return rows matching your query
exactly.

Finally, use the -R/--reverse flag if data is sorted in descending order and
the -N/--numeric flag if data is sorted numerically rather than lexicographically.

Examples:

Searching for rows matching exactly "Anna" in a "name" column:

    $ xan bisect -S name Anna people.csv

Finding all names starting with letter A:

    $ xan bisect name A people.csv | xan slice -E '!name.startswith("A")'

Usage:
    xan bisect [options] [--] <column> <value> <input>
    xan bisect --help

bisect options:
    -S, --search   Perform an exact search and only emit rows matching the
                   query, instead of flushing all rows from found position.
    -R, --reverse  Indicate that the file is sorted on <column> in descending
                   order, instead of the default ascending order.
    -N, --numeric  Indicate that searched values are numbers and that the order
                   of the file is numerical instead of default lexicographic
                   order.
    -E, --exclude  When set, rows matching query exactly will be filtered out.
                   It is equivalent to performing the "upper bound" operation
                   but it does not come with the same performance guarantees
                   in case there are many rows containing the searched values.
                   Does not work with -S/--search.
    -v, --verbose  Print some log detailing the search process in stderr, mostly
                   for debugging purposes.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
