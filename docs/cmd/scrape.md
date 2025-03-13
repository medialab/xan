<!-- Generated -->
# xan scrape

```txt
Scrape HTML using a CSS-like expression language.

TODO...

Usage:
    xan scrape -e <expr> <column> [options] [<input>]
    xan scrape title <column> [options] [<input>]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>    If given, evaluate the given scraping expression.
    -f, --foreach <css>      If given, will return one row per element matching
                             the CSS selector in target document, instead of returning
                             a single row per document.
    -I, --input-dir <path>   If given, target column will be understood
                             as relative path to read from this input
                             directory instead.
    -k, --keep <column>      Selection of columns from the input to keep in
                             the output.
    --sep <char>             Separator to use when serializing lists.
                             [default: |]
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
