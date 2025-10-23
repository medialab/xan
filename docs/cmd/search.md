<!-- Generated -->
# xan search

```txt
Search for (or replace) patterns in CSV data (be sure to check out `xan grep` for
a faster but coarser equivalent).

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

# Searching multiple patterns at once

This command is also able to search for multiple patterns at once.
To do so, you can either use the -P, --add-pattern flag or feed a text file
with one pattern per line to the --patterns flag. You can also feed a CSV file
to the --patterns flag, in which case you will need to indicate the column
containing the patterns using the --pattern-column flag.

Giving additional patterns:

    $ xan search disc -P tape -P vinyl file.csv > matches.csv

One pattern per line of text file:

    $ xan search --patterns patterns.txt file.csv > matches.csv

CSV column containing patterns:

    $ xan search --patterns people.csv --pattern-column name tweets.csv > matches.csv

Feeding patterns through stdin (using "-"):

    $ cat patterns.txt | xan search --patterns - file.csv > matches.csv

Feeding CSV column as patterns through stdin (using "-"):

    $ xan slice -l 10 people.csv | xan search --patterns - --pattern-column name file.csv > matches.csv

# Further than just filtering

Now this command is also able to perform search-adjacent operations:

    - Replacing matches with -R/--replace or --replacement-column
    - Reporting in a new column whether a match was found with -f/--flag
    - Reporting the total number of matches in a new column with -c/--count
    - Reporting a breakdown of number of matches per query given through --patterns
      with -B/--breakdown.
    - Reporting unique matches of multiple queries given through --patterns
      using -U/--unique-matches.

For instance:

Reporting whether a match was found (instead of filtering):

    $ xan search -s headline -i france -f france_match file.csv

Reporting number of matches:

    $ xan search -s headline -i france -c france_count file.csv

Cleaning thousands separators (usually commas "," in English) from numerical columns:

    $ xan search , --replace . -s 'count_*' file.csv

Replacing color names to their French counterpart:

    $ echo 'english,french\nred,rouge\ngreen,vert' | \
    $ xan search -e \
    $   --patterns - --pattern-column english --replacement-column french \
    $   -s color file.csv > translated.csv

Computing a breakdown of matches per query:

    $ xan search -B -s headline --patterns queries.csv \
    $   --pattern-column query --name-column name file.csv > breakdown.csv

Reporting unique matches per query in a new column:

    $ xan search -U matches -s headline,text --patterns queries.csv \
    $   --pattern-column query --name-column name file.csv > matches.csv

# Regarding parallelization

Finally, this command can leverage multithreading to run faster using
the -p/--parallel or -t/--threads flags. This said, the boost given by
parallelization might differ a lot and depends on the complexity and number of
queries and also on the size of the haystacks. That is to say `xan search --empty`
would not be significantly faster when parallelized whereas `xan search -i eternity`
definitely would.

Also, you might want to try `xan parallel cat` instead because it could be
faster in some scenarios at the cost of an increase in memory usage (and it
won't work on streams and unindexed gzipped data).

For instance, the following `search` command:

    $ xan search -i eternity -p file.csv

Would directly translate to:

    $ xan parallel cat -P 'search -i eternity' -F file.csv

Usage:
    xan search [options] --non-empty [<input>]
    xan search [options] --empty [<input>]
    xan search [options] --patterns <index> [<input>]
    xan search [options] <pattern> [-P <pattern>...] [<input>]
    xan search --help

search mode options:
    -e, --exact       Perform an exact match.
    -r, --regex       Use a regex to perform the match.
    -E, --empty       Search for empty cells, i.e. filter out
                      any completely non-empty selection.
    -N, --non-empty   Search for non-empty cells, i.e. filter out
                      any completely empty selection.
    -u, --url-prefix  Match by url prefix, i.e. cells must contain urls
                      matching the searched url prefix. Urls are first
                      reordered using a scheme called a LRU, that you can
                      read about here:
                      https://github.com/medialab/ural?tab=readme-ov-file#about-lrus

search options:
    -i, --ignore-case        Case insensitive search.
    -v, --invert-match       Select only rows that did not match
    -s, --select <arg>       Select the columns to search. See 'xan select -h'
                             for the full syntax.
    -A, --all                Only return a row when ALL columns from the given selection
                             match the desired pattern, instead of returning a row
                             when ANY column matches.
    -f, --flag <column>      Instead of filtering rows, add a new column indicating if any match
                             was found.
    -c, --count <column>     Report the number of non-overlapping pattern matches in a new column with
                             given name. Will still filter out rows with 0 matches, unless --left
                             is used. Does not work with -v/--invert-match.
    --overlapping            When used with -c/--count or -B/--breakdown, return the count of
                             overlapping matches. Note that this can sometimes be one order of
                             magnitude slower that counting non-overlapping matches.
    -R, --replace <with>     If given, the command will not filter rows but will instead
                             replace matches with the given replacement.
                             Does not work with --replacement-column.
                             Regex replacement string syntax can be found here:
                             https://docs.rs/regex/latest/regex/struct.Regex.html#replacement-string-syntax
    -l, --limit <n>          Maximum of number rows to return. Useful to avoid downstream
                             buffering some times (e.g. when searching for very few
                             rows in a big file before piping to `view` or `flatten`).
                             Does not work with -p/--parallel nor -t/--threads.
    --left                   Rows without any matches will be kept in the output when
                             using -U/--unique-matches, or -B/--breakdown, or -c/--count.
    -p, --parallel           Whether to use parallelization to speed up computation.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

multiple patterns options:
    -P, --add-pattern <pattern>  Manually add patterns to query without needing to feed a file
                                 to the --patterns flag.
    -B, --breakdown              When used with --patterns, will count the total number of
                                 non-overlapping matches per pattern and write this count in
                                 one additional column per pattern. Added column will be given
                                 the pattern as name, unless you provide the --name-column flag.
                                 Will not include rows that have no matches in the output, unless
                                 the --left flag is used. You might want to use it with --overlapping
                                 sometimes when your patterns are themselves overlapping or you might
                                 be surprised by the tallies.
    -U, --unique-matches <name>  When used with --patterns, will add a column containing a list of
                                 unique matched patterns for each row, separated by the --sep character.
                                 Will not include rows that have no matches in the output unless
                                 the --left flag is used. Patterns can also be given a name through
                                 the --name-column flag.
    --sep <char>                 Character to use to join pattern matches when using -U/--unique-matches.
                                 [default: |]
    --patterns <path>            Path to a text file (use "-" for stdin), containing multiple
                                 patterns, one per line, to search at once.
    --pattern-column <name>      When given a column name, --patterns file will be considered a CSV
                                 and patterns to search will be extracted from the given column.
    --replacement-column <name>  When given with both --patterns & --pattern-column, indicates the
                                 column containing a replacement when a match occurs. Does not
                                 work with -R/--replace.
                                 Regex replacement string syntax can be found here:
                                 https://docs.rs/regex/latest/regex/struct.Regex.html#replacement-string-syntax
    --name-column <name>         When given with -B/--breakdown, --patterns & --pattern-column,
                                 indicates the column containing a pattern's name that will be used
                                 as column name in the appended breakdown.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
