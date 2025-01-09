<!-- Generated -->
# xan fill

```txt
Fill empty cells of a CSV file by filling them with any non-empty value seen
before (this is usually called forward filling), or with any constant value
given to the -v, --value flag.

For instance, replacing empty values with 0 everywhere in the file:

    $ xan fill -v 0 data.csv > filled.csv

Usage:
    xan fill [options] [<input>]
    xan fill --help

fill options:
    -s, --select <cols>  Selection of columns to fill.
    -v, --value <value>  Fill empty cells using provided value instead of using
                         last non-empty value.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
