<!-- Generated -->
# xan head

```txt
Return the first rows of a CSV file.

An alias for `xan slice -l/--len <n>`.

Usage:
    xan head [options] [<input>]

head options:
    --rows <n>  Number of rows to return. [default: 10]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Otherwise, the first row will always
                           appear in the output as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
