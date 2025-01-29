<!-- Generated -->
# xan join

```txt
Join two sets of CSV data on the specified columns.

The default join operation is an "inner" join. This corresponds to the
intersection of rows on the keys specified. The command is also able to
perform a left outer join with --left, a right outer join with --right,
a full outer join with --full and finally a cartesian product/cross join
with --cross.

By default, joins are done case sensitively, but this can be disabled using
the -i, --ignore-case flag.

The column arguments specify the columns to join for each input. Columns can
be selected using the same syntax as the "xan select" command. Both selections
must return a same number of columns, for the join keys to be properly aligned.

Note that this command is able to consume streams such as stdin (in which case
the file name must be "-" to indicate which file will be read from stdin) and
gzipped files out of the box.

# Memory considerations

    - `inner join`: the command does not try to be clever and
                    always indexes the left file, while the right
                    file is streamed. Prefer placing the smaller file
                    on the left.
    - `left join`:  the command always indexes the right file and streams
                    the left file.
    - `right join`: the command always indexes the left file and streams
                    the right file.
    - `full join`:  the command does not try to be clever and
                    always indexes the left file, while the right
                    file is streamed. Prefer placing the smaller file
                    on the left.
    - `cross join`: the command does not try to be clever and
                    always indexes the left file, while the right
                    file is streamed. Prefer placing the smaller file
                    on the left.

Usage:
    xan join [options] <columns1> <input1> <columns2> <input2>
    xan join [options] --cross <input1> <input2>
    xan join --help

join options:
    --left                       Do an "outer left" join. This returns all rows in
                                 first CSV data set, including rows with no
                                 corresponding row in the second data set. When no
                                 corresponding row exists, it is padded out with
                                 empty fields. This is the reverse of --right.
    --right                      Do an "outer right" join. This returns all rows in
                                 second CSV data set, including rows with no
                                 corresponding row in the first data set. When no
                                 corresponding row exists, it is padded out with
                                 empty fields. This is the reverse of --left.
    --full                       Do a "full outer" join. This returns all rows in
                                 both data sets with matching records joined. If
                                 there is no match, the missing side will be padded
                                 out with empty fields.
    --cross                      This returns the cartesian product of the given CSV
                                 files. The number of rows emitted will be equal to N * M,
                                 where N and M correspond to the number of rows in the given
                                 data sets, respectively.
    -i, --ignore-case            When set, joins are done case insensitively.
    --nulls                      When set, joins will work on empty fields.
                                 Otherwise, empty keys are completely ignored, i.e. when
                                 column selection yield only empty cells.
    -L, --prefix-left <prefix>   Add a prefix to the names of the columns in the
                                 first dataset.
    -R, --prefix-right <prefix>  Add a prefix to the names of the columns in the
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
