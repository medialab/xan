<!-- Generated -->
# xan complete

```txt
Complete or check on missing values in a column. Can handle integer or date values.
A --min and/or --max flag can be used to specify a range to complete or check.
Note that when completing, if the input contains values outside the specified
range, those values will be removed from the output.
You can specify that the input is already sorted in ascending order on the column
to complete with the --sorted flag, and in descending order using both --sorted
and --reverse, which will make the command faster.
Will by default output in ascending order on the completed column, but you can
use the --reverse flag to output in descending order.
You can also complete values within groups defined by other columns using the --groupby
flag, completing with the same range for each group.

Examples:
  Complete integer values in column named "score" from 1 to 10:
    $ xan complete -m 1 -M 10 score input.csv

  Complete already sorted date values in column named "date":
    $ xan complete -D --sorted date input.csv

  Check that the values (already sorted in descending order) in column named "score" are complete:
    $ xan complete --check --sorted --reverse score input.csv

  Complete integer values in column named "score" within groups defined by columns "name" and "category":
    $ xan complete --groupby name,category score input.csv

Usage:
    xan complete [options] <column> [<input>]
    xan complete --help

complete options:
    -m, --min <value>        The minimum value to start completing from.
                             Default is the first one. Note that if <value> is
                             greater than the minimum value in the input, the
                             rows with values lower than <value> will be removed
                             from the output.
    -M, --max <value>        The maximum value to complete to.
                             Default is the last one. Note that if <value> is
                             lower than the maximum value in the input, the rows
                             with values greater than <value> will be removed
                             from the output.
    --check                  Check that the input is complete. When used with
                             either --min or --max, only checks completeness
                             within the specified range.
    -D, --dates              Set to indicate your values are dates (supporting
                             year, year-month or year-month-day).
    --sorted                 Indicate that the input is already sorted. When
                             used without --reverse, the input is sorted in
                             ascending order. When used with --reverse, the
                             input is sorted in descending order.
    -R, --reverse            When used with --sorted, indicate that the input is
                             sorted in descending order. When used
                             without --sorted, the output will be sorted in
                             descending order.
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
