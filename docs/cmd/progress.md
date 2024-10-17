<!-- Generated -->
# xan progress

```txt
Display a progress bar while reading the rows of a CSV file.

The command will try and buffer some of the ingested file to find
the total number of rows automatically. If you know the total
beforehand, you can also use the --total flag.

Usage:
    xan progress [options] [<input>]
    xan progress --help

progress options:
    -S, --smooth         Flush output buffer each time one row is written.
                         This makes the progress bar smoother, but might be
                         less performant.
    -B, --bytes          Display progress on file bytes, rather than parsing CSV lines.
    --prebuffer <n>      Number of megabytes of the file to prebuffer to attempt
                         knowing the progress bar total automatically.
                         [default: 64]
    --title <string>     Title of the loading bar.
    --total <n>          Total number of rows of given CSV file.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will be included in
                           the progress bar total.
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
