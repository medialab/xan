<!-- Generated -->
# xan url-join

```txt
Join a CSV file containing a column of url prefixes with another CSV file.

The default behavior of this command is to be an 'inner join', which
means only matched rows will be written in the output. Use the --left
flag if you want to perform a 'left join' and keep every row of the searched
file in the output.

The file containing urls will always be completely read in memory
while the second one will always be streamed.

You can of course work on gzipped files if needed and feed one of both
files from stdin by using `-` instead of a path.

Not that this command indexes the hierarchical reordering of a bunch of urls
into a prefix tree. This reordering scheme is named LRUs and you can read about
it here: https://github.com/medialab/ural#about-lrus

If you only need to filter rows of the second file and don't
actually need to join columns from the urls file, you should
probably use `xan search --url-prefix --patterns` instead.

Usage:
    xan url-join [options] <column> <input> <url-column> <urls>
    xan url-join --help

join options:
    -S, --simplified             Drop irrelevant parts of the urls, like the scheme,
                                 `www.` subdomains etc. to facilitate matches.
    --left                       Write every row from input file in the output, with empty
                                 padding cells on the right when no url from the second
                                 file produced any match.
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
