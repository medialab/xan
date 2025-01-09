<!-- Generated -->
# xan cat

```txt
Concatenates CSV data by column or by row.

When concatenating by column, the columns will be written in the same order as
the inputs given. The number of rows in the result is always equivalent to to
the minimum number of rows across all given CSV data. (This behavior can be
reversed with the '--pad' flag.)

When concatenating by row, all CSV data must have the same number of columns.
If you need to rearrange the columns or fix the lengths of records, use the
'select' or 'fixlengths' commands. Also, only the headers of the *first* CSV
data given are used. Headers in subsequent inputs are ignored. (This behavior
can be disabled with --no-headers.)

When concatenating a large number of CSV files exceeding your shell's
command arguments limit, prefer using the --paths flag to read the list of CSV
files to concatenate from input lines or from a CSV file containing paths in a
column given to the --path-column flag.

Feeding --paths lines:

    $ xan cat rows --paths paths.txt > concatenated.csv

Feeding --paths CSV file:

    $ xan cat rows --paths files.csv --path-column path > concatenated.csv

Feeding stdin ("-") to --paths:

    $ find . -name '*.csv' | xan cat rows --paths - > concatenated.csv

Feeding CSV as stdin ("-") to --paths (typically using `xan glob`):

    $ xan glob '**/*.csv' | xan cat rows --paths - --path-column path > concatenated.csv

Usage:
    xan cat rows    [options] [<inputs>...]
    xan cat columns [options] [<inputs>...]
    xan cat --help

cat columns options:
    -p, --pad                   When concatenating columns, this flag will cause
                                all records to appear. It will pad each row if
                                other CSV data isn't long enough.

cat rows options:
    --paths <input>             When concatenating rows, give a text file (use "-" for stdin)
                                containing one path of CSV file to concatenate per line.
    --path-column <name>        When given a column name, --paths will be considered as CSV, and paths
                                to CSV files to concatenate will be extracted from the selected column.
    -S, --source-column <name>  Name of a column to prepend in the output of "cat rows"
                                indicating the path to source file.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be interpreted
                           as column names. Note that this has no effect when
                           concatenating columns.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
