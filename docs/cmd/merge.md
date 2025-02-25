<!-- Generated -->
# xan merge

```txt
Merge multiple CSV files already sorted the same way. Those files MUST:

1. have the same columns in the same order.
2. have the same row order wrt -s/--select, -R/--reverse & -N/--numeric

If those conditions are not met, the result will be in arbitrary order.

This command consumes memory proportional to one CSV row per file.

When merging a large number of CSV files exceeding your shell's
command arguments limit, prefer using the --paths flag to read the list of CSV
files to merge from input lines or from a CSV file containing paths in a
column given to the --path-column flag.

Note that all the files will need to be opened at once, so you might hit the
maximum number of opened files of your OS then.

Feeding --paths lines:

    $ xan merge --paths paths.txt > merged.csv

Feeding --paths CSV file:

    $ xan merge --paths files.csv --path-column path > merged.csv

Feeding stdin ("-") to --paths:

    $ find . -name '*.csv' | xan merge --paths - > merged.csv

Feeding CSV as stdin ("-") to --paths:

    $ cat filelist.csv | xan merge --paths - --path-column path > merged.csv

Usage:
    xan merge [options] [<inputs>...]
    xan merge --help

merge options:
    -s, --select <arg>          Select a subset of columns to sort.
                                See 'xan select --help' for the format details.
    -N, --numeric               Compare according to string numerical value
    -R, --reverse               Reverse order
    -u, --uniq                  When set, identical consecutive lines will be dropped
                                to keep only one line per sorted value.
    -S, --source-column <name>  Name of a column to prepend in the output of the command
                                indicating the path to source file.
    --paths <input>             Give a text file (use "-" for stdin) containing one path of
                                CSV file to concatenate per line, instead of giving the paths
                                through the command's arguments.
    --path-column <name>        When given a column name, --paths will be considered as CSV, and paths
                                to CSV files to merge will be extracted from the selected column.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
