<!-- Generated -->
# xan search

```txt
Search for (or replace) patterns in CSV data. That is to say keep rows of given
CSV file if ANY of the selected column matches the given pattern or patterns.

This command has several flags to select the way to perform a match:

    * (default): matching a substring (e.g. "john" in "My name is john")
    * -e, --exact: exact match
    * -r, --regex: using a regular expression
    * -u, --url-prefix: matching by url prefix (e.g. "lemonde.fr/business")
    * -N, --non-empty: finding non-empty cells (does not need a pattern)
    * -E, --empty: finding empty cells (does not need a pattern)

Searching for rows with any column containing "john":

    $ xan search "john" file.csv > matches.csv

Searching for rows where any column has *exactly* the value "john":

    $ xan search -e "john" file.csv > matches.csv

Keeping only rows where selection is not fully empty:

    $ xan search -s user_id --non-empty file.csv > users-with-id.csv

Keeping only rows where selection has any empty column:

    $ xan search -s user_id --empty file.csv > users-without-id.csv

When using a regular expression, be sure to mind bash escape rules (prefer single
quotes around your expression and don't forget to use backslashes when needed):

    $ xan search -r '\bfran[cç]' file.csv

To restrict the columns that will be searched you can use the -s, --select flag.

All search modes (except -u/--url-prefix) can also be case-insensitive
using -i, --ignore-case.

This command is also able to search for multiple patterns at once.
To do so, you must give a text file with one pattern per line to the --patterns
flag, or a CSV file containing a column of to indicate using --pattern-column.

One pattern per line of text file:

    $ xan search --patterns patterns.txt file.csv > matches.csv

CSV column containing patterns:

    $ xan search --patterns people.csv --pattern-column name tweets.csv > matches.csv

Feeding patterns through stdin (using "-"):

    $ cat patterns.txt | xan search --patterns - file.csv > matches.csv

Feeding CSV column as patterns through stdin (using "-"):

    $ xan slice -l 10 people.csv | xan search --patterns - --pattern-column name file.csv > matches.csv

This command can also count the number of matches and report it in a new column,
using the -c/--count flag.

Finally, this command is able to replace matched values through the -R/--replace
flag and the --replacement-column flag when combined with --patterns & --pattern-column.

Cleaning thousands separators (usually commas "," in English) from numerical columns:

    $ xan search , --replace . -s 'count_*' file.csv

Replacing color names to their French counterpart:

    $ echo 'english,french\nred,rouge\ngreen,vert' | \
    $ xan search -e \
    $   --patterns - --pattern-column english --replacement-column french \
    $   -s color file.csv > translated.csv

Usage:
    xan search [options] --non-empty [<input>]
    xan search [options] --empty [<input>]
    xan search [options] --patterns <index> [<input>]
    xan search [options] <pattern> [<input>]
    xan search --help

search options:
    -e, --exact                  Perform an exact match.
    -r, --regex                  Use a regex to perform the match.
    -E, --empty                  Search for empty cells, i.e. filter out
                                 any completely non-empty selection.
    -N, --non-empty              Search for non-empty cells, i.e. filter out
                                 any completely empty selection.
    -u, --url-prefix             Match by url prefix, i.e. cells must contain urls
                                 matching the searched url prefix. Urls are first
                                 reordered using a scheme called a LRU, that you can
                                 read about here:
                                 https://github.com/medialab/ural?tab=readme-ov-file#about-lrus
    -i, --ignore-case            Case insensitive search.
    -s, --select <arg>           Select the columns to search. See 'xan select -h'
                                 for the full syntax.
    -v, --invert-match           Select only rows that did not match
    -A, --all                    Only return a row when ALL columns from the given selection
                                 match the desired pattern, instead of returning a row
                                 when ANY column matches.
    -c, --count <column>         If given, the command will not filter rows but will instead
                                 count the total number of non-overlapping pattern matches per
                                 row and report it in a new column with given name.
                                 Does not work with -v/--invert-match.
    -B, --breakdown              When used with --patterns, will count the total number of
                                 non-overlapping matches per pattern and write this count in
                                 one additional column per pattern. You might want to use
                                 it with --overlapping sometimes when your patterns are themselves
                                 overlapping.
    --overlapping                When used with -c/--count or -B/--breakdown, return the count of
                                 overlapping matches. Note that this can sometimes be one order of
                                 magnitude slower that counting non-overlapping matches.
    -R, --replace <with>         If given, the command will not filter rows but will instead
                                 replace matches with the given replacement.
                                 Does not work with --replacement-column.
    --patterns <path>            Path to a text file (use "-" for stdin), containing multiple
                                 patterns, one per line, to search at once.
    --pattern-column <name>      When given a column name, --patterns file will be considered a CSV
                                 and patterns to search will be extracted from the given column.
    --replacement-column <name>  When given with both --patterns & --pattern-column, indicates the
                                 column containing a replacement when a match occurs. Does not
                                 work with -R/--replace.
    --name-column <name>         When given with -B/--breakdown, --patterns & --pattern-column,
                                 indicates the column containing a pattern's name that will be used
                                 as column name in the appended breakdown.
    -l, --limit <n>              Maximum of number rows to return. Useful to avoid downstream
                                 buffering some times (e.g. when searching for very few
                                 rows in a big file before piping to `view` or `flatten`).

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
