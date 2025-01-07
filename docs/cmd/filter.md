<!-- Generated -->
# xan filter

```txt
The filter command evaluates an expression for each row of the given CSV file and
only output the row if the result of beforementioned expression is truthy.

For instance, given the following CSV file:

a
1
2
3

The following command:

    $ xan filter 'a > 1'

Will produce the following result:

a
2
3

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan filter [options] <expression> [<input>]
    xan filter --cheatsheet
    xan filter --functions
    xan filter --help

filter options:
    -p, --parallel             Whether to use parallelization to speed up computations.
                               Will automatically select a suitable number of threads to use
                               based on your number of cores. Use -t, --threads if you want to
                               indicate the number of threads yourself.
    -t, --threads <threads>    Parellize computations using this many threads. Use -p, --parallel
                               if you want the number of threads to be automatically chosen instead.
    -v, --invert-match         If set, will invert the evaluated value.
    -l, --limit <n>            Maximum number of rows to return. Useful to avoid downstream
                               buffering some times (e.g. when searching for very few
                               rows in a big file before piping to `view` or `flatten`).
                               Does not work when parallelizing.
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
