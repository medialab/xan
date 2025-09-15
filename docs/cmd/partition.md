<!-- Generated -->
# xan partition

```txt
Partition the given CSV data into chunks based on the values of a column.

The files are written to the output directory with filenames based on the
values in the partition column and the `--filename` flag.

By default, this command will consider it works in a case-insensitive filesystem
(e.g. on macOS). This can have an impact on the names of the create files. If you
know beforehand that your filesystem is case-sensitive and want filenames to be
better aligned with the original values use the -C/--case-sensitive flag.

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
    -C, --case-sensitive       Don't perform case normalization to assess whether a
                               new file has to be created when seeing a new value.
                               Only use on case-sensitive filesystems or this can have
                               adverse effects!

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Otherwise, the first row will
                           appear in all chunks as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
