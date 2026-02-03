<!-- Generated -->
# xan separate

```txt
Separate a single column into multiple ones by splitting its cells according
to some splitting algorithm that can be one of:

    * (default): splitting by a single substring
    * -r, --regex: splitting using a regular expression
    * -m, --match: decomposing into regular expression matches
    * -c, --capture-groups: decomposing into regular expression capture groups
    * --fixed-width: cutting every <n> bytes
    * --widths: split by a list of consecutive widths
    * --cuts: cut at predefined byte offsets
    * --offsets: extract byte slices

Created columns can be given a name using the --into flag, else they will be
given generic names based on the original column name. For instance, splitting a
column named "text" will produce columns named "text1", "text2"...

It is also possible to limit the number of splits using the --max flag.

If the number of splits is known beforehand (that is to say when using --into
or --max or --widths or --cuts or --offsets), the command will be able to stream
the data. Else it will have to buffer the whole file into memory to record the
maximum number of splits produced by the selected method.

Finally, note that by default, the separated column will be removed from the output,
unless the -k/--keep flag is used.

Examples:

  Splitting a full name
    $ xan separate fullname ' ' data.csv
    $ xan separate --into first_name,last_name ' ' data.csv

  Splitting a full name using a regular expression
    $ xan separate -r fullname '\s+' data.csv

  Extracting digit sequences from a column named 'birthdate' using a regex:
    $ xan separate -r -m birthdate '\d+' data.csv

  Extracting year, month and day from a column named 'date' using capture groups:
    $ xan separate date '(\d{4})-(\d{2})-(\d{2})' data.csv -r -c --into year,month,day

  Splitting a column named 'code' into sequences of 3 bytes:
    $ xan separate code --fixed-width 3 data.csv

  Splitting a column named 'code' into parts of widths 2, 4 and 3:
    $ xan separate code --widths 2,4,3 data.csv

  Splitting a column named 'code' on bytes 2 and 6:
    $ xan separate code --cuts 2,6 data.csv

  Split column named 'code' into of segments defined by byte offsets [0, 2), [2, 6) and [6, 9):
    $ xan separate code --offsets 0,2,6,9 data.csv

Usage:
    xan separate [options] <column> <separator> [<input>]
    xan separate --help

separate mode options:
    -r, --regex           When using --separator, split cells using a regular
                          expression instead of a simple substring.
    -m, --match           When using -r/--regex, extract parts of the cell matching
                          the regex pattern.
    -c, --capture-groups  When using -r/--regex, extract parts of the call matching
                          the regex pattern's capture groups.
    --fixed-width         Split cells every <separator> bytes. Each resulting part
                          will then be trimmed of leading/trailing whitespace.
    --widths              Split cells using the given widths (given as a comma-separated
                          list of integers). Each resulting part will then be trimmed of
                          leading/trailing whitespace.
    --cuts                Split cells on the given bytes (given as a comma-separated
                          list of increasing, non-repeating integers). Each resulting part
                          will then be trimmed of leading/trailing whitespace.
    --offsets             Split cells according to the specified byte offsets (given as a
                          comma-separated list of increasing, non-repeating integers).
                          Each resulting part will then be trimmed of leading/trailing whitespace.

separate options:
    -M, --max <n>          Limit the number of cells splitted to at most <n>.
                           By default, all possible splits are made.
    --into <column-names>  Specify names for the new columns created by the
                           splits. If not provided, new columns will be named
                           before the original column name ('text' column will
                           be separated into 'text1', 'text2', etc.). If used with --max,
                           the number of names provided must be equal or lower
                           than <n>. Cannot be used with --prefix.
    --prefix <prefix>      Specify a prefix for the new columns created by the
                           splits. By default, no prefix is used and new columns
                           are named before the original column name ('text'
                           column will be separated into 'text1', 'text2', etc.).
                           Cannot be used with --into.
    --too-many <option>    Specify how to handle extra cells when the number
                           of splitted cells exceeds --max, or
                           the number of provided names with --into.
                           Must be one of:
                                - 'error': stop as soon as an inconsistent number
                                    of splits is produced.
                                - 'drop': drop splits over expected maximum.
                                - 'merge': append the rest of the cell to the last
                                    produced split.
                           Note that 'merge' cannot be used with -m/--match
                           nor -c/--capture-groups.
                           [default: error]
    -k, --keep             Keep the separated column after splitting, instead of
                           discarding it.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
```
