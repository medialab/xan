<!-- Generated -->
# xan map

```txt
The map command evaluates an expression for each row of the given CSV file and
output the same row with added columns containing the results of beforementioned
expression.

For instance, given the following CSV file:

a,b
1,4
5,2

The following command:

    $ xan map 'a + b as c' file.csv > result.csv

Will produce the following result:

a,b,c
1,4,5
5,2,7

You can also create multiple columns at once:

    $ xan map 'a + b as c, a * b as d' file.csv > result.csv

Will produce the following result:

a,b,c,d
1,4,5,4
5,2,7,10

You can also use the -O/--overwrite flag to overwrite already existing columns:

    $ xan map -O 'b * 10 as b, a * b as c' file.csv > result.csv

Will produce:

a,b,c
1,40,4
5,20,10

The expression can optionally be read from a file using the -f/--evaluate-file flag:

    $ xan map -f expr.moonblade file.csv > result.csv

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Miscellaneous tricks:

1. Copying a column:

    $ xan map 'column_name as copy_name' file.csv > result.csv

2. Create a column containing a constant value:

    $ xan map '"john" as from' file.csv > result.csv

Usage:
    xan map [options] <expression> [<input>]
    xan map --help

map options:
    -f, --evaluate-file        Read evaluation expression from a file instead.
    -O, --overwrite            If set, expressions named with a column already existing
                               in the file will be overwritten with the result of the
                               expression instead of adding a new column at the end.
                               This means you can both transform and add columns at the
                               same time.
    -F, --filter               If given, will not write rows in the output if the result
                               of evaluated expression is falsey. Will only work when
                               given expression has a single clause.
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
