<!-- Generated -->
# xan flatten

```txt
Prints flattened records such that fields are labeled separated by a new line.
This mode is particularly useful for viewing one record at a time.

There is also a condensed view (-c or --condense) that will shorten the
contents of each field to provide a summary view.

Pipe into "less -r" if you need to page the result, and use "-C, --force-colors"
not to lose the colors:

    $ xan flatten -C file.csv | less -r

Usage:
    xan flatten [options] [<input>]
    xan f [options] [<input>]

flatten options:
    -s, --select <arg>     Select the columns to visualize. See 'xan select -h'
                           for the full syntax.
    -c, --condense         Don't wrap cell values on new lines but truncate them
                           with ellipsis instead.
    -w, --wrap             Wrap cell values all while minding the header's indent.
    --cols <num>           Width of the graph in terminal columns, i.e. characters.
                           Defaults to using all your terminal's width or 80 if
                           terminal's size cannot be found (i.e. when piping to file).
    -R, --rainbow          Alternating colors for cells, rather than color by value type.
    -C, --force-colors     Force colors even if output is not supposed to be able to
                           handle them.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. When set, the name of each field
                           will be its index.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```