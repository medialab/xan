<!-- Generated -->
# xan flatmap

```txt
The flatmap command evaluates an expression for each row of the given CSV file.
This expression is expected to return a potentially iterable value (e.g. a list).

If said value is falsey, then no row will be written in the output of the input
row.

Then, for each nested value yielded by the expression, one row of CSV will be
written to the output.

This row will have the same columns as the input with an additional one
containing the nested value or replacing the value of a column of your choice,
using the -r/--replace flag.

For instance, given the following CSV file:

name,colors
John,blue
Mary,yellow|red

The following command:

    $ xan flatmap 'split(colors, "|")' color -r colors

Will produce the following result:

name,color
John,blue
Mary,yellow
Mary,red

Note that if the expression returns an empty list or a falsey value, no row will
be written in the output for the current input row. This means one can use the
flatmap command as a sort of combined map and filter in a single pass over the CSV file.

For instance, given the following CSV file:

name,age
John Mayer,34
Mary Sue,45

The following command:

    $ xan flatmap 'if(age >= 40, last(split(name, " ")))' surname

Will produce the following result:

name,age,surname
Mary Sue,45,Sue

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan flatmap [options] <expression> <column> [<input>]
    xan flatmap --help

flatmap options:
    -r, --replace <column>     Name of the column that will be replaced by the mapped values.
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.
    -E, --errors <policy>      What to do with evaluation errors. One of:
                                 - "panic": exit on first error
                                 - "ignore": coerce result for row to null
                                 - "log": print error to stderr
                               [default: panic].

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
