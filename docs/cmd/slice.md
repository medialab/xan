<!-- Generated -->
# xan slice

```txt
Returns rows of a CSV file in the specified range. This range can be specified
through 0-based rows indices, byte offsets in the file and using custom expressions
as start & stop conditions.

Slicing the 10 first rows of a file:

    $ xan slice -l 10 file.csv

Slicing rows between indices 5 and 10:

    $ xan slice -s 5 -e 10 file.csv

Retrieving rows at some indices:

    $ xan slice -I 4,5,19,65 file.csv

Slicing rows starting at some byte offset in the file:

    $ xan slice -B 56356 file.csv

Slicing rows until a row where the "count" column is over `45`:

    $ xan slice -E 'count > 45' file.csv

The command will of course terminate as soon as the specified range of rows is
found and won't need to read to whole file or stream if unnecessary.

Of course, flags related to byte offsets will only work with seekable inputs, e.g. files
on disk but no stdin nor gzipped files.

Note that it is perfectly fine to mix & match flags related to row indices,
byte offsets and conditions. In which case, here is description of the order
of operations:

- First, the command will seek in target file if -B/--byte-offset was given, and
won't read past a certain byte offset if --end-byte was given.
- Then the -S/--start-condition and -E/--end-condtion apply.
- Finally flags related to row indices will apply. Note that indices are therefore
relative to both the application of the byte offset and the start condition and not
to the first actual row in the file.

So, for instance, if you want to slice 5 rows in the file but only after a row
where the "count" column is over `10`, you could do the following:

    $ xan slice -S 'count > 10' -l 5 file.csv

Usage:
    xan slice [options] [<input>]

slice options to use with row indices:
    -s, --start <n>    The index of the record to slice from.
    --skip <n>         Same as -s, --start.
    -e, --end <n>      The index of the record to slice to.
    -l, --len <n>      The length of the slice (can be used instead of --end).
    -i, --index <i>    Slice a single record (shortcut for -s N -l 1).
    -I, --indices <i>  Return a slice containing multiple indices at once.
                       You must provide the indices separated by commas,
                       e.g. "1,4,67,89". Note that selected records will be
                       emitted in file order, not in the order given.

slice options to use with expressions:
    -S, --start-condition <expr>  Do not start yielding rows until given expression
                                  returns true.
    -E, --end-condition <expr>    Stop yielding rows as soon as given expression
                                  returns false.

slice options to use with byte offets:
    -B, --byte-offset <b>  Byte offset to seek to in the sliced file. This can
                           be useful to access a particular slice of records in
                           constant time, without needing to read preceding bytes.
                           You must provide a byte offset starting a CSV record or
                           the output could be corrupted. This requires the input
                           to be seekable (stdin or gzipped files not supported).
    --end-byte <b>         Only read up to provided position in byte, exclusive.
                           This requires the input to be seekable (stdin or gzipped
                           files not supported).

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Otherwise, the first row will always
                           appear in the output as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
