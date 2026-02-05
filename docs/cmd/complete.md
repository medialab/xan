<!-- Generated -->
# xan complete

```txt
Complete CSV data by adding rows for missing values of a given column.

This command is able to handle either integer or partial dates (year-month-date,
year-month or just year).

A --min and/or --max flag can be used to specify a range to complete. Note that
if input contains values outside of the specified range, they will be filtered
out from the output.

If you know your input is already sorted on the column to complete, you can
leverage the -S/--sorted flag to make the command work faster and use less
memory.

This command is also able to check whether the given column is complete using
the --check flag.

Examples:

Complete integer column named "score" from 1 to 10:
    $ xan complete -m 1 -M 10 score input.csv

Complete already sorted date values in column named "date":
    $ xan complete -D --sorted date input.csv

Check completeness of values (already sorted in descending order) in "score" column:
    $ xan complete --check --sorted --reverse score input.csv

Complete integer column named "score" within groups defined by columns "name" and "category":
    $ xan complete --groupby name,category score input.csv

Usage:
    xan complete [options] <column> [<input>]
    xan complete --help

complete options:
    --check                  Check that the input is complete. When used with
                             either --min or --max, only checks completeness
                             within the specified range.
    -m, --min <value>        Minimum value of range to complete. Note that values
                             less than this minimum value in the input will be
                             filtered out.
    -M, --max <value>        Maximum value of range to complete. Note that values
                             greater than this maximum value in the input will be
                             filtered out.
    -D, --dates              Set to indicate your values are dates (supporting
                             year, year-month or year-month-day).
    -S, --sorted             Indicate that the input is already sorted.
    -R, --reverse            Whether to consider the data in reverse order.
    -g, --groupby <cols>     Select columns to group by. The completion will be
                             done independently within each group.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
