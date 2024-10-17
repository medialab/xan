<!-- Generated -->
# xan implode

```txt
Implode a CSV file by collapsing multiple consecutive rows into a single one
where the values of some columns are joined using the given separator.

This is the reverse of the 'explode' command.

For instance the following CSV:

name,color
John,blue
John,yellow
Mary,red

Can be imploded on the "color" column using the "|" <separator> to produce:

name,color
John,blue|yellow
Mary,red

Usage:
    xan implode [options] <columns> <separator> [<input>]
    xan implode --help

implode options:
    -r, --rename <name>    New name for the diverging column.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
