<!-- Generated -->
# xan explode

```txt
Explode CSV rows into multiple ones by splitting selected cell using the pipe
character ("|") or any separator given to the --sep flag.

This is conceptually the inverse of the "implode" command.

For instance the following CSV:

*file.csv*
name,colors
John,blue|yellow
Mary,red

Can be exploded on the "colors" column:

    $ xan explode colors --singular file.csv > exploded.csv

To produce the following file:

*exploded.csv*
name,color
John,blue
John,yellow
Mary,red

Note finally that the file can be exploded on multiple well-aligned columns (that
is to say selected cells must all be splitted into a same number of values).

Usage:
    xan explode [options] <columns> [<input>]
    xan explode --help

explode options:
    --sep <sep>          Separator to split the cells.
                         [default: |]
    -S, --singularize    Singularize (supporting only very simple English-centric cases)
                         the exploded column names. Does not work with -r, --rename.
    -r, --rename <name>  New names for the exploded columns. Must be written
                         in CSV format if exploding multiple columns.
                         See 'xan rename' help for more details.
                         Does not work with -S, --singular.
    -D, --drop-empty     Drop rows when selected cells are empty.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
