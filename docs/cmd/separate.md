<!-- Generated -->
# xan separate

```txt
Separate ONE column into multiple columns by splitting cell values on a separator or regex.
By default, all possible splits are made, but you can limit the number of splits
using the --max-splitted-cells option.
Note that by default, the original column is removed from the output. Use the --keep-column
flag to retain it.

This command takes the specified column and splits each cell in that column using either
a substring separator or a regex pattern. The resulting parts are output as new columns.
You can choose to split by a simple substring or use a regex for more complex splitting.
Additional options allow you to extract only matching parts, or capture groups from the regex.

Examples:

  Split column named 'fullname' on space:
    $ xan separate fullname ' ' data.csv

  Split column named 'fullname' on whitespaces using a regex:
    $ xan separate -r fullname '\s+' data.csv

  Extract digit sequences from column named 'birthdate' as separate columns using a regex:
    $ xan separate -r -m birthdate '\d+' data.csv

  Extract year, month and day from column named 'date' using capture groups:
    $ xan separate date '(\d{4})-(\d{2})-(\d{2})' data.csv -r -c --into year,month,day

  Split column 'code' into parts of fixed width 3:
    $ xan separate code --fixed-width 3 data.csv

  Split column 'code' into parts of widths 2,4,3:
    $ xan separate code --widths 2,4,3 data.csv

  Split column 'code' on bytes 2,6:
    $ xan separate code --split-on-bytes 2,6 data.csv

  Split column 'code' into parts of segments defined by offsets 0,2,6,9 (same as
  split-on-bytes 2,6 if the length of the cell is 9):
    $ xan separate code --segment-bytes 0,2,6,9 data.csv

Usage:
    xan separate [options] <column> <separator> [<input>]
    xan separate --help

separate options:
    -k, --keep                Keep the separated column after splitting.
    --max-splitted-cells <n>  Limit the number of cells splitted to at most <n>.
                              By default, all possible splits are made.
    --into <column-names>     Specify names for the new columns created by the
                              splits. If not provided, new columns will be named
                              split1, split2, etc. If used with --max-splitted-cells,
                              the number of names provided must be equal or lower
                              than <n>.
    --too-many <option>       Specify how to handle extra cells when the number
                              of splitted cells exceeds --max-splitted-cells, or
                              the number of provided names with --into.
                              By default, it will cause an error. Options are 'drop'
                              to discard extra parts, or 'merge' to combine them
                              into the last column. Note that 'merge' cannot be
                              used with -m/--match nor -c/--capture-groups.
                              [default: error]
    -r, --regex               When using --separator, split cells using <separator>
                              as a regex pattern instead of splitting.
    -m, --match               When using -r/--regex, only output the parts of the
                              cell that match the regex pattern. By default, the
                              parts between matches (i.e. separators) are output.
    -c, --capture-groups      When using -r/--regex, if the regex contains capture
                              groups, output the text matching each capture group
                              as a separate column.
    --fixed-width             Split cells every <separator> bytes. Cannot be used
                              with --widths, --split-on-bytes nor --segment-bytes.
                              Trims whitespace for each splitted cell.
    --widths                  Split cells using the specified fixed widths
                              (comma-separated list of integers). Cannot be
                              used with --fixed-width, --split-on-bytes, --segment-bytes
                              nor --max-splitted-cells. Trims whitespace for each
                              splitted cell.
    --split-on-bytes          Split cells on the specified bytes
                              (comma-separated list of integers). Cannot be used
                              with --fixed-width, --widths, --segment-bytes
                              nor --max-splitted-cells. Trims whitespace for each
                              splitted cell.
    --segment-bytes           Split cells according to the specified byte offsets
                              (comma-separated list of integers). Cannot be used
                              with --fixed-width, --widths, --split-on-bytes
                              nor --max-splitted-cells. Trims whitespace for
                              each splitted cell. When the first byte is 0 and
                              the last byte is equal to the cell length,
                              this is equivalent to --split-on-bytes (we're being
                              more explicit here).

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
