<!-- Generated -->
# xan fuzzy-join

```txt
Join a CSV file containing a column of patterns that will be matched with rows
of another CSV file.

This command has several flags to select the way to perform matches:

    * (default): matching a substring (e.g. "john" in "My name is john")
    * -r, --regex: using a regular expression
    * -u, --url-prefix: matching by url prefix (e.g. "lemonde.fr/business")

The default behavior of this command is to do an 'inner join', which
means only matched rows will be written in the output. Use the --left
flag if you want to perform a 'left join' and keep every row of the searched
file in the output.

The file containing patterns will always be completely read in memory
while the second one will always be streamed.

You can of course work on gzipped files if needed and feed one of both
files from stdin by using `-` instead of a path.

Fuzzy-join is a costly operation, especially when testing a large number of patterns,
so a -p/--parallel and -t/--threads flag can be used to use multiple CPUs and
speed up the search.

A typical use-case for this command is to fuzzy search family
names, using regex patterns, in some text column of a CSV file, all while
keeping any match-related column from the pattern file.

This said, if you only need to filter rows of the second file and don't
actually need to join columns from the patterns file, you should
probably use `xan search --regex --patterns` instead.

Usage:
    xan fuzzy-join [options] <columns> <input> <pattern-columns> <patterns>
    xan fuzzy-join --help

fuzzy-join options:
    -r, --regex                  Join by regex patterns.
    -u, --url-prefix             Join by url prefix, i.e. cells must contain urls
                                 matching the searched url prefix. Urls are first
                                 reordered using a scheme called a LRU, that you can
                                 read about here:
                                 https://github.com/medialab/ural?tab=readme-ov-file#about-lrus
    -i, --ignore-case            Make the patterns case-insensitive.
    -S, --simplified             When using -u/--url-prefix, drop irrelevant parts of the urls,
                                 like the scheme, `www.` subdomains etc. to facilitate matches.
    --left                       Write every row from input file in the output, with empty
                                 padding cells on the right when no regex pattern from the second
                                 file produced any match.
    -p, --parallel               Whether to use parallelization to speed up computations.
                                 Will automatically select a suitable number of threads to use
                                 based on your number of cores. Use -t, --threads if you want to
                                 indicate the number of threads yourself.
    -t, --threads <threads>      Parellize computations using this many threads. Use -p, --parallel
                                 if you want the number of threads to be automatically chosen instead.
    -L, --prefix-left <prefix>   Add a prefix to the names of the columns in the
                                 searched file.
    -R, --prefix-right <prefix>  Add a prefix to the names of the columns in the
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
