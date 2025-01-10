<!-- Generated -->
# xan join

```txt
Joins two sets of CSV data on the specified columns.

The default join operation is an 'inner' join. This corresponds to the
intersection of rows on the keys specified.

By default, joins are done case sensitively, but this can be disabled using
the --ignore-case flag.

The column arguments specify the columns to join for each input. Columns can
be selected using the same syntax as the 'xan select' command. Both selections
must return a same number of columns in proper order.

Note that this command is able to consume streams such as stdin (in which case
the file name must be '-' to indicate which file will be read from stdin) and
gzipped files out of the box, but be aware that those file will be entirely
buffered into memory so the join operation can be done.

Note that when performing an 'inner' join (the default), it's the second file that
will be indexed into memory. And when performing an 'outer' join, it will be the file
that is on the other side of --left/--right.

Finally, the command can also perform a 'regex' join, matching efficiently a CSV file containing
a column of regex patterns with another file. But if you only need to filter out a file
based on a set of regex patterns and don't need the auxilliary columns to be concatenated
to the joined result, please be sure to check out the search command --patterns flag before.

Usage:
    xan join [options] <columns1> <input1> <columns2> <input2>
    xan join --help

join options:
    -i, --ignore-case           When set, joins are done case insensitively.
    --left                      Do a 'left outer' join. This returns all rows in
                                first CSV data set, including rows with no
                                corresponding row in the second data set. When no
                                corresponding row exists, it is padded out with
                                empty fields.
    --right                     Do a 'right outer' join. This returns all rows in
                                second CSV data set, including rows with no
                                corresponding row in the first data set. When no
                                corresponding row exists, it is padded out with
                                empty fields. (This is the reverse of 'outer left'.)
    --full                      Do a 'full outer' join. This returns all rows in
                                both data sets with matching records joined. If
                                there is no match, the missing side will be padded
                                out with empty fields. (This is the combination of
                                'outer left' and 'outer right'.)
    --cross                     USE WITH CAUTION.
                                This returns the cartesian product of the CSV
                                data sets given. The number of rows return is
                                equal to N * M, where N and M correspond to the
                                number of rows in the given data sets, respectively.
    --regex                     Perform an optimized regex join where the second file
                                contains a column of regex patterns that will be used
                                to match the values of a column of the first file.
                                This is a variant of 'inner join' in that only matching
                                rows will be written to the output.
    --regex-left                Perform an optimized regex join where the second file
                                contains a column of regex patterns that will be used
                                to match the values of a column of the first file.
                                This is a variant of 'left join' in that all rows from
                                the first files will be written at least one, even if
                                no pattern from the second file matched.
    -p, --parallel              Whether to use parallelization to speed up computations.
                                Will automatically select a suitable number of threads to use
                                based on your number of cores. Use -t, --threads if you want to
                                indicate the number of threads yourself. Only works with --regex
                                and --regex-left currently.
    -t, --threads <threads>     Parellize computations using this many threads. Use -p, --parallel
                                if you want the number of threads to be automatically chosen instead.
                                Only works with --regex and --regex-left currently.
    --nulls                     When set, joins will work on empty fields.
                                Otherwise, empty fields are completely ignored.
                                (In fact, any row that has an empty field in the
                                key specified is ignored.)
    --prefix-left <prefix>      Add a prefix to the names of the columns in the
                                first dataset.
    --prefix-right <prefix>     Add a prefix to the names of the columns in the
                                second dataset.

Common options:
    -h, --help                  Display this message
    -o, --output <file>         Write output to <file> instead of stdout.
    -n, --no-headers            When set, the first row will not be interpreted
                                as headers. (i.e., They are not searched, analyzed,
                                sliced, etc.)
    -d, --delimiter <arg>       The field delimiter for reading CSV data.
                                Must be a single character.
```
