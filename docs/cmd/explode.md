<!-- Generated -->
# xan explode

```txt
Explode CSV rows into multiple ones by splitting column values by using the
provided separator.

This is the reverse of the 'implode' command.

For instance the following CSV:

name,colors
John,blue|yellow
Mary,red

Can be exploded on the "colors" column using the "|" <separator> to produce:

name,colors
John,blue
John,yellow
Mary,red

Note finally that the file can be exploded on multiple well-aligned columns.

Usage:
    xan explode [options] <columns> <separator> [<input>]
    xan explode --help

explode options:
    -r, --rename <name>    New names for the exploded columns. Must be written
                           in CSV format if exploding multiple columns.
                           See 'xan rename' help for more details.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
