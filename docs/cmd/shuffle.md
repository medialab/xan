<!-- Generated -->
# xan shuffle

```txt
Shuffle the given CSV file. Requires memory proportional to the
number of rows of the file (approx. 2 u64 per row).

Note that rows from input file are copied as-is in the output.
This means that no CSV serialization harmonization will happen,
unless --in-memory is set.

Also, since this command needs random access in the input file, it
does not work with stdin or piping (unless --in-memory) is set.

Usage:
    xan shuffle [options] [<input>]
    xan shuffle --help

shuffle options:
    --seed <number>        RNG seed.
    -m, --in-memory        Load all CSV data in memory before shuffling it. Can
                           be useful for streamed inputs such as stdin but of
                           course costs more memory.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
