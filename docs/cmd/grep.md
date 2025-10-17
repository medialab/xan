<!-- Generated -->
# xan grep

```txt
Keep rows of a CSV file matching a given pattern. It can be thought of as
a CSV-aware version of the well-known `grep` command.

This command is faster than `xan search` because it relies on an optimized CSV
parser that only knows how to separate rows and does not care about finding cell
delimitations. But this also means this command has less features and is less
precise than `xan search` because it will try to match the given pattern on whole
rows at once, quotes & delimiters included. This is usually not an issue for coarse
filtering, but keep in mind it could be problematic for your use case.

Note also that if your CSV data has no quoting whatsoever, you really should
use `ripgrep` instead:
https://github.com/BurntSushi/ripgrep

Finally, contrary to most `xan` commands that will normalize the output to
standardish CSV data with commas and quoting using double quotes, this command
will output rows as-is, without any transformation.

Usage:
    xan grep [options] <pattern> [<input>]
    xan grep --help

grep options:
    -c, --count         Only return the number of matching rows.
    -r, --regex         Matches the given pattern as a regex.
    -i, --ignore-case   Ignore case while matching rows.
    -v, --invert-match  Only return or count rows that did not match
                        given pattern.
    --mmap              Use a memory map to speed up computations. Only
                        works if the file is on disk (no streams) and if the
                        file is uncompressed. Usually a bad idea on macOS.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. Otherwise, the first row will always
                           appear in the output as the header row.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
