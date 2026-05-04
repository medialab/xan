<!-- Generated -->
# xan explode

```txt
Explode CSV rows into multiple ones by splitting selected cell using the pipe
character ("|") or any separator given to the --sep flag.

This command is conceptually the inverse of the "implode" command.

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

Note that a file can be exploded on multiple well-aligned columns that would
be split into a same number of values. Else you can always use the --pad flag.

Alternatively, you can also use an expression using the -e <expr> flag if you
need to split your cells using custom logic, e.g. parsing JSON etc.

    $ xan explode json_names -e '_.parse_json().compact()' file.csv

Usage:
    xan explode [options] <columns> [<input>]
    xan explode --help

explode options:
    --sep <sep>            Separator to split the cells.
                           [default: |]
    -e, --evaluate <expr>  Evaluate an expression to split cells instead of using
                           a simple separator.
    -f, --evaluate-file <path>
                           Read splitting expression from a file instead.
    -S, --singularize      Singularize (supporting only very simple English-centric cases)
                           the exploded column names. Does not work with -r, --rename.
    -r, --rename <name>    New names for the exploded columns. Must be written
                           in CSV format if exploding multiple columns.
                           See 'xan rename' help for more details.
                           Does not work with -S, --singular.
    -k, --keep             Keep the exploded columns alongside each split.
    -D, --drop-empty       Drop rows when selected cells are empty.
    --pad                  When exploding multiple columns at once, pad shorter splits
                           to align them with the longest one instead of erroring.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
