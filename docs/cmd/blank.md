<!-- Generated -->
# xan blank

```txt
Blank down selected columns of a CSV file. That is to say, this
command will redact any consecutive identical cells as per column selection.

This can be useful as a presentation trick or a compression scheme.

The "blank" term comes from OpenRefine and does the same thing.

Usage:
    xan blank [options] [<input>]
    xan blank --help

blank options:
    -s, --select <cols>    Selection of columns to blank down.
    -r, --redact <value>   Redact the blanked down values using the provided
                           replacement string. Will default to an empty string.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
