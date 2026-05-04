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

# Fuzzy join

This command is also able to perform a so-called "fuzzy" join using the
following flags:

    * -c, --contains: matching a substring (e.g. "john" in "My name is john")
    * -r, --regex: using a regular expression
    * -u, --url-prefix: matching by url prefix (e.g. "lemonde.fr/business")

The file containing patterns has to be, by convention, given on the right, while
the left one should contain values that will be tested against those patterns.

This means --left can still be used to emit rows without any match.

Fuzzy-join is a costly operation, especially when testing a large number of patterns,
so a -p/--parallel and -t/--threads flag can be used to use multiple CPUs and
speed up the search.

A typical use-case for this command is to fuzzy search family
names, using regex patterns, in some text column of a CSV file, all while
keeping any match-related column from the pattern file.

This said, if you only need to filter rows of the second file and don't
actually need to join columns from the patterns file, you should
probably use `xan search --patterns` instead.

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
    - `fuzzy join`: the command always indexes patterns of the right file and
                    streams the file on the left.

Usage:
    xan join [options] <columns1> <input1> <columns2> <input2>
    xan join [options] <columns> <input1> <input2>
    xan join [options] --cross <input1> <input2>
    xan join --help

join mode options:
    --inner  Do an "inner" join. This only returns rows where
             a match can be found between both data sets. This
             is the command's default, so this flag can be omitted,
             or used for clarity.
    --left   Do an "outer left" join. This returns all rows in
             first CSV data set, including rows with no
             corresponding row in the second data set. When no
             corresponding row exists, it is padded out with
             empty fields. This is the reverse of --right.
             Can be used in fuzzy joins.
    --right  Do an "outer right" join. This returns all rows in
             second CSV data set, including rows with no
             corresponding row in the first data set. When no
             corresponding row exists, it is padded out with
             empty fields. This is the reverse of --left.
    --full   Do a "full outer" join. This returns all rows in
             both data sets with matching records joined. If
             there is no match, the missing side will be padded
             out with empty fields.
    --semi   Only keep rows of left file matching a row in right file.
    --anti   Only keep rows of left file not matching a row in right file.
    --cross  This returns the cartesian product of the given CSV
             files. The number of rows emitted will be equal to N * M,
             where N and M correspond to the number of rows in the given
             data sets, respectively.

fuzzy join mode options:
    -c, --contains    Join by matching substrings.
    -r, --regex       Join by regex patterns.
    -u, --url-prefix  Join by url prefix, i.e. cells must contain urls
                      matching the searched url prefix. Urls are first
                      reordered using a scheme called a LRU, that you can
                      read about here:
                      https://github.com/medialab/ural?tab=readme-ov-file#about-lrus

join options:
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

fuzzy join options:
    -S, --simplified-urls    When using -u/--url-prefix, drop irrelevant parts of the urls,
                             like the scheme, `www.` subdomains etc. to facilitate matches.
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help                  Display this message
    -o, --output <file>         Write output to <file> instead of stdout.
    -n, --no-headers            When set, the first row will not be interpreted
                                as headers. (i.e., They are not searched, analyzed,
                                sliced, etc.)
    -d, --delimiter <arg>       The field delimiter for reading CSV data.
                                Must be a single character.
```
