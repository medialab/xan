<!-- Generated -->
# xan regex-join

```txt
Join a CSV file containing a column of regex patterns with another CSV file.

The default behavior of this command is to be an 'inner join', which
means only matched rows will be written in the output. Use the --left
flag if you want to perform a 'left join' and keep every row of the searched
file in the output.

The file containing patterns will always be completely read in memory
while the second one will always be streamed.

You can of course work on gzipped files if needed and feed one of both
files from stdin by using `-` instead of a path.

Note that this commands relies on a regexset under the hood and is
more performant than just testing every regex pattern for each row
of the other CSV file.

This remains a costly operation, especially when testing a large
number of regex patterns, so a -p/--parallel and -t/--threads
flag can be used to use multiple CPUs and speed up the search.

A typical use-case for this command is to fuzzy search family
names, using regex patterns, in some text column of a CSV file, all while
keeping any match-related column from the pattern file.

This said, if you only need to filter rows of the second file and don't
actually need to join columns from the patterns file, you should
probably use `xan search --patterns` instead.

Usage:
    xan regex-join [options] <columns> <input> <pattern-col> <patterns-input>
    xan regex-join --help

join options:
    -i, --ignore-case           Make the regex patterns case-insensitive.
    --left                      Write every row from the first file in the output, with empty
                                padding cells when no regex pattern from the second file
                                produced a match.
    -p, --parallel              Whether to use parallelization to speed up computations.
                                Will automatically select a suitable number of threads to use
                                based on your number of cores. Use -t, --threads if you want to
                                indicate the number of threads yourself.
    -t, --threads <threads>     Parellize computations using this many threads. Use -p, --parallel
                                if you want the number of threads to be automatically chosen instead.
    --prefix-left <prefix>      Add a prefix to the names of the columns in the
                                searched file.
    --prefix-right <prefix>     Add a prefix to the names of the columns in the
                                patterns file.

Common options:
    -h, --help                  Display this message
    -o, --output <file>         Write output to <file> instead of stdout.
    -n, --no-headers            When set, the first row will not be interpreted
                                as headers. (i.e., They are not searched, analyzed,
                                sliced, etc.)
    -d, --delimiter <arg>       The field delimiter for reading CSV data.
                                Must be a single character.
```
