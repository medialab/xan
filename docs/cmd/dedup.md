<!-- Generated -->
# xan dedup

```txt
Deduplicate the rows of a CSV file. Runs in O(n) time, consuming O(c) memory, c being
the distinct number of row identities.

If your file is already sorted on the deduplication selection, use the -S/--sorted flag
to run in O(1) memory instead.

Note that, by default, this command will write the first row having
a specific identity to the output, unless you use -l/--keep-last.

The command can also write only the duplicated rows with --keep-duplicates.

Finally, it is also possible to specify which rows to keep by evaluating
an expression (see `xan help cheatsheet` and `xan help functions` for
the documentation of the expression language).

For instance, if you want to deduplicate a CSV of events on the `id`
column but want to keep the row having the maximum value in the `count`
column instead of the first row found with any given identity:

    $ xan dedup -s id --choose 'new_count > current_count' events.csv > deduped.csv

Notice how the column names of the currently kept row were prefixed
with "current_", while the ones of the new row were prefixed
with "new_" instead.

Note that if you need to aggregate cell values from duplicated
rows, you should probably check out `xan groupby` instead, that can
be used for this very purpose, especially with the --keep flag.

Usage:
    xan dedup [options] [<input>]
    xan dedup --help

dedup options:
    --check             Verify whether the selection has any duplicates, i.e. whether
                        the selected columns satisfy a uniqueness constraint.
    -s, --select <arg>  Select a subset of columns to on which to deduplicate.
                        See 'xan select --help' for the format details.
    -S, --sorted        Use if you know your file is already sorted on the deduplication
                        selection to avoid needing to keep a hashmap of values
                        in memory.
    -l, --keep-last     Keep the last row having a specific identity, rather than
                        the first one. Note that it will cost more memory and that
                        no rows will be flushed before the whole file has been read
                        if -S/--sorted is not used.
    -e, --external      Use an external btree index to keep the index on disk and avoid
                        overflowing RAM. Does not work with -l/--keep-last and --keep-duplicates.
    --keep-duplicates   Emit only the duplicated rows.
    --choose <expr>     Evaluate an expression that must return whether to
                        keep a newly seen row or not. Column name in the given
                        expression will be prefixed with "current_" for the
                        currently kept row and "new_" for the new row to consider.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
