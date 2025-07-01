<!-- Generated -->
# xan transform

```txt
The transform command can be used to edit a selection of columns for each row
of a CSV file using a custom expression.

For instance, given the following CSV file:

name,surname
john,davis
mary,sue

The following command (notice how `_` is used as a reference to the currently
edited column):

    $ xan transform surname 'upper(_)'

Will produce the following result:

name,surname
john,DAVIS
mary,SUE

When using unary functions, the above command can be written even shorter:

    $ xan transfrom surname upper

The above example work on a single column but the command is perfectly able to
transform multiple columns at once using a selection:

    $ xan transform name,surname,fullname upper

For a quick review of the capabilities of the expression language,
check out the `xan help cheatsheet` command.

For a list of available functions, use `xan help functions`.

Usage:
    xan transform [options] <column> <expression> [<input>]
    xan transform --help

transform options:
    -r, --rename <name>        New name for the transformed column.
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
