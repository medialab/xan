<!-- Generated -->
# xan implode

```txt
Implode a CSV file by merging multiple consecutive rows into a single one, where
diverging cells will be joined by the pipe character ("|") or any separator
given to the --sep flag.

This is the inverse of the "explode" command.

For instance the following CSV:

*file.csv*
name,color
John,blue
John,yellow
Mary,red

Can be imploded on the "color" column:

    $ xan implode color --plural file.csv > imploded.csv

To produce the following file:

*imploded.csv*
name,color
John,blue|yellow
Mary,red

Usage:
    xan implode [options] <columns> [<input>]
    xan implode --help

implode options:
    --sep <sep>          Separator that will be used to join the diverging cells.
                         [default: |]
    -P, --plural         Adding a final "s" to the imploded column names.
                         Does not work with -r, --rename.
    -r, --rename <name>  New name for the diverging column.
                         Does not work with -P, --plural.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
