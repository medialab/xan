<!-- Generated -->
# xan join

```txt
Join two CSV files on the specified columns.

The default join operation is an "inner" join. This corresponds to the
intersection of rows on the keys specified. The command is also able to
perform a left outer join with --left, a right outer join with --right,
a full outer join with --full, a semi join with --semi, an anti join with --anti
and finally a cartesian product/cross join with --cross.

By default, joins are done case sensitively, but this can be changed using
the -i, --ignore-case flag.

The column arguments specify the columns to join for each input. Columns can
be selected using the same syntax as the "xan select" command. Both selections
must return a same number of columns, for the join keys to be properly aligned.

Note that when it is obviously safe to drop the joined columns from one of the files
the command will do so automatically. Else you can tweak the command's behavior
using the -D/--drop-key flag.

Note that this command is able to consume streams such as stdin (in which case
the file name must be "-" to indicate which file will be read from stdin).

# Examples

Inner join of two files on a column named differently:

    $ xan join user_id tweets.csv id accounts.csv > joined.csv

The same, but with columns named the same:

    $ xan join user_id tweets.csv accounts.csv > joined.csv

Left join:

    $ xan join --left user_id tweets.csv id accounts.csv > joined.csv

Joining on multiple columns:

    $ xan join media,month per-query.csv totals.csv > joined.csv

One file from stdin:

    $ xan filter 'retweets > 10' tweets.csv | xan join user_id - id accounts.csv > joined.csv

Prefixing right column names:

    $ xan join -R user_ user_id tweets.csv id accounts.csv > joined.csv

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
    - `semi join`:  the command always indexes the right file and streams the
                    left file.
    - `anti join`:  the command always indexes the right file and streams the
                    left file.
    - `cross join`: the command does not try to be clever and
                    always indexes the left file, while the right
                    file is streamed. Prefer placing the smaller file
                    on the left.

Usage:
    xan join [options] <columns1> <input1> <columns2> <input2>
    xan join [options] <columns> <input1> <input2>
    xan join [options] --cross <input1> <input2>
    xan join --help

join options:
    --inner                      Do an "inner" join. This only returns rows where
                                 a match can be found between both data sets. This
                                 is the command's default, so this flag can be omitted,
                                 or used for clarity.
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
    --semi                       Only keep rows of left file matching a row in right file.
    --anti                       Only keep rows of left file not matching a row in right file.
    --cross                      This returns the cartesian product of the given CSV
                                 files. The number of rows emitted will be equal to N * M,
                                 where N and M correspond to the number of rows in the given
                                 data sets, respectively.
    -i, --ignore-case            When set, joins are done case insensitively.
    --nulls                      When set, joins will work on empty fields.
                                 Otherwise, empty keys are completely ignored, i.e. when
                                 column selection yield only empty cells.
    -D, --drop-key <mode>        Indicate whether to drop columns representing the join key
                                 in `left` or `right` file, or `none`, or `both`.
                                 Defaults to `none` unless joined columns are named the same
                                 and -i, --ignore-case is not set.
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
