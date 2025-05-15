<!-- Generated -->
# xan partition

```txt
Partitions the given CSV data into chunks based on the value of a column

The files are written to the output directory with filenames based on the
values in the partition column and the `--filename` flag.

Note that most operating systems avoid opening more than 1024 files at once,
so if you know the cardinality of the paritioned column is very high, please
sort the file on this column beforehand and use the -S/--sorted flag.

Usage:
    xan partition [options] <column> [<input>]
    xan partition --help

partition options:
    -O, --out-dir <dir>        Where to write the chunks. Defaults to current working
                               directory.
    -f, --filename <filename>  A filename template to use when constructing
                               the names of the output files.  The string '{}'
                               will be replaced by a value based on the value
                               of the field, but sanitized for shell safety.
                               [default: {}.csv]
    -p, --prefix-length <n>    Truncate the partition column after the
                               specified number of bytes when creating the
                               output file.
    -S, --sorted               Use this flag if you know the file is sorted
                               on the partition column in advance, so the command
                               can run faster and with less memory and resources
                               opened.
    --drop                     Drop the partition column from results.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Otherwise, the first row will
                           appear in all chunks as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
