<!-- Generated -->
# xan map

```txt
The map command evaluates an expression for each row of the given CSV file and
output the row with an added column containing the result of beforementioned
expression.

For instance, given the following CSV file:

a,b
1,4
5,2

The following command:

    $ xan map 'a + b' c > result.csv

Will produce the following result:

a,b,c
1,4,5
5,2,7

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

If you want to list available functions, use the --functions flag.

Miscellaneous tricks:

1. Copying a column:

    $ xan map 'column_name' copy_name > result.csv

2. Create a column containing a constant value:

    $ xan map '"john"' from > result.csv

Usage:
    xan map [options] <expression> <column> [<input>]
    xan map --cheatsheet
    xan map --functions
    xan map --help

map options:
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.
    -E, --errors <policy>      What to do with evaluation errors. One of:
                                 - "panic": exit on first error
                                 - "report": add a column containing error
                                 - "ignore": coerce result for row to null
                                 - "log": print error to stderr
                               [default: panic].
    --error-column <name>      Name of the column containing errors if -E/--errors
                               is set to "report".
                               [default: xan_error].

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
