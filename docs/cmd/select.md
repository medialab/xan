<!-- Generated -->
# xan select

```txt
Select columns from CSV data efficiently using either a handy DSL or by
evaluating an expression on each row (using the -e, --evaluate flag).

This command lets you manipulate the columns in CSV data. You can re-order
them, duplicate them, transform them or drop them.

1. Selection DSL:
-----------------

Columns can be referenced by zero-based index, by negative index starting
from the end, by name (if the file has headers) and by name and nth, so you
can easily select columns by duplicate names.

Finally, this is also possible to select ranges of columns using the `:`
character. Note that column range are inclusive.

Examples:

  Select the first and fourth columns:
    $ xan select 0,3

  Select the last column using negative indexing (mind the `--`
  to avoid shell issues with values starting with hyphens):
    $ xan select -- -1

  Select first and next to last:
    $ xan select 0,-2

  Select the first 4 columns (by index and by name):
    $ xan select 0:3
    $ xan select Header1:Header4

  Ignore the first 2 columns (by range and by omission):
    $ xan select 2:
    $ xan select '!0:1' (use single quotes to avoid shell issues!)

  Select using negative indices in range:
    $ xan select 3:-2 (fourth to next to last)
    $ xan select -- -3: (last three columns)
    $ xan select :-3 (up to the third from last)

  Select the third column named 'Foo':
    $ xan select 'Foo[2]'

  Select the last column named 'Foo':
    $ xan select 'Foo[-1]'

  Select column names containing spaces:
    $ xan select "Revenues in millions"
    $ xan select 1,"Revenues in millions",year

  Re-order and duplicate columns arbitrarily:
    $ xan select 3:1,Header3:Header1,Header1,Foo[2],Header1

  Quote column names that conflict with selector syntax,
  (mind the double quoting, problematic characters being `:`, `!`, `[` and `]`):
    $ xan select '"Start:datetime","Count:int"'

  Select all the columns which is useful to add some copies of columns
  (notice the simple quotes to avoid shell-side globbing):
    $ xan select '*'
    $ xan select '*,name'
    $ xan select '*,1'
    $ xan select '0:'
    $ xan select ':0'

2. Evaluating a expression:
---------------------------

Using a SQLish syntax that is the same as for the `map`, `agg`, `filter` etc.
commands, you can wrangle the rows and perform a custom selection.

  $ xan select -e 'name, prenom as surname, count1 + count2 as total'

You can also use the -A/--append flag to perform something akin to
multiple `xan map` commands piped together:

  $ xan select -Ae 'a + b as c, len(name) as name_len'

For a quick review of the capabilities of the script language, use
the --cheatsheet flag.

If you want to list available functions, use the --functions flag.

Usage:
    xan select [options] [--] <selection> [<input>]
    xan select --help
    xan select --cheatsheet
    xan select --functions

select options:
    -A, --append           Append the selection to the rows instead of
                           replacing them.
    -e, --evaluate         Toggle expression evaluation rather than using the DSL.
    -E, --errors <policy>  What to do with evaluation errors. One of:
                             - "panic": exit on first error
                             - "ignore": ignore row altogether
                             - "log": print error to stderr
                           [default: panic].

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
