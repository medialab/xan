<!-- Generated -->
# xan search

```txt
Filter rows of given CSV file if some of its cells contains a desired substring.

Can also be used to search for exact matches using the -e, --exact flag.

Can also be used to search using a regular expression using the -r, --regex flag.

When using a regular expression, be sure to mind bash escape rules (prefer single
quotes around your expression and don't forget to use backslashes when needed):

    $ xan search -r '\bfran[cç]' file.csv

To restrict the columns that will be searched you can use the -s, --select flag.

All search modes can also be case-insensitive using -i, --ignore-case.

Finally, this command is also able to take a CSV file column containing multiple
patterns to search for at once, using the --input flag:

    $ xan search user_id --input user-ids.csv tweets.csv

Usage:
    xan search [options] <column> --input <index> [<input>]
    xan search [options] <pattern> [<input>]
    xan search --help

search options:
    -e, --exact            Perform an exact match.
    -r, --regex            Use a regex to perform the match.
    --input <index>        CSV file containing a column of value to index & search.
    -i, --ignore-case      Case insensitive search. This is equivalent to
                           prefixing the regex with '(?i)'.
    -s, --select <arg>     Select the columns to search. See 'xan select -h'
                           for the full syntax.
    -v, --invert-match     Select only rows that did not match
    -f, --flag <column>    If given, the command will not filter rows
                           but will instead flag the found rows in a new
                           column with given name.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
