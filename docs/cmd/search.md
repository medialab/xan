<!-- Generated -->
# xan search

```txt
Keep rows of given CSV file if ANY of the selected columns contains a desired
substring.

Can also be used to search for exact matches using the -e, --exact flag.

Can also be used to search using a regular expression using the -r, --regex flag.

Can also be used to search for empty or non-empty selections. For instance,
keeping only rows where selection is not fully empty:

    $ xan search --non-empty file.csv

Or keeping only rows where selection has any empty column:

    $ xan search --empty file.csv

When using a regular expression, be sure to mind bash escape rules (prefer single
quotes around your expression and don't forget to use backslashes when needed):

    $ xan search -r '\bfran[cç]' file.csv

To restrict the columns that will be searched you can use the -s, --select flag.

All search modes can also be case-insensitive using -i, --ignore-case.

Finally, this command is also able to search for multiple patterns at once.
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

Usage:
    xan search [options] --non-empty [<input>]
    xan search [options] --empty [<input>]
    xan search [options] --patterns <index> [<input>]
    xan search [options] <pattern> [<input>]
    xan search --help

search options:
    -e, --exact              Perform an exact match.
    -r, --regex              Use a regex to perform the match.
    -E, --empty              Search for empty cells, i.e. filter out
                             any completely non-empty selection.
    -N, --non-empty          Search for non-empty cells, i.e. filter out
                             any completely empty selection.
    --patterns <path>        Path to a text file (use "-" for stdin), containing multiple
                             patterns, one per line, to search at once.
    --pattern-column <name>  When given a column name, --patterns file will be considered a CSV
                             and patterns to search will be extracted from the given column.
    -i, --ignore-case        Case insensitive search.
    -s, --select <arg>       Select the columns to search. See 'xan select -h'
                             for the full syntax.
    -v, --invert-match       Select only rows that did not match
    -A, --all                Only return a row when ALL columns from the given selection
                             match the desired pattern, instead of returning a row
                             when ANY column matches.
    -c, --count <column>     If given, the command will not filter rows but will instead
                             count the total number of non-overlapping pattern matches per
                             row and report it in a new column with given name.
                             Does not work with -v/--invert-match.
    -l, --limit <n>          Maximum of number rows to return. Useful to avoid downstream
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
