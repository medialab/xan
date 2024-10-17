<!-- Generated -->
# xan search

```txt
Filters CSV data by whether the given pattern matches a row.

By default, the pattern is a regex and is applied to each field in each row,
and if any field matches, then the row is written to the output. The columns to search
can be limited with the '-s, --select' flag (but the full row is still written to the
output if there is a match).

The pattern can also be an exact match, case sensitive or not.

The command is also able to take a CSV file column containing multiple
patterns as an input. This can be thought of as a specialized kind
of left join over the data.

When giving a regex, be sure to mind bash escape rules (prefer single quotes
around your expression and don't forget to use backslash when needed).

Usage:
    xan search [options] <column> --input <index> [<input>]
    xan search [options] <pattern> [<input>]
    xan search --help

search options:
    -e, --exact            Perform an exact match rather than using a
                           regular expression.
    --input <index>        CSV file containing a column of value to index & search.
    -i, --ignore-case      Case insensitive search. This is equivalent to
                           prefixing the regex with '(?i)'.
    -s, --select <arg>     Select the columns to search. See 'xan select -h'
                           for the full syntax.
    -v, --invert-match     Select only rows that did not match

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
    -f, --flag <column>    If given, the command will not filter rows
                           but will instead flag the found rows in a new
                           column named <column>.
```
