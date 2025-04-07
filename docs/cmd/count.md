<!-- Generated -->
# xan count

```txt
Prints a count of the number of records in the CSV data.

Note that the count will not include the header row (unless --no-headers is
given).

Usage:
    xan count [options] [<input>]

count options:
    --csv  Output the result as a single column, single row CSV file with
           a "count" header.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
