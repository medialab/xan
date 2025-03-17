<!-- Generated -->
# xan scrape

```txt
Scrape HTML files to output tabular CSV data.

This command can either process a CSV file with a column containing
raw HTML, or a CSV file with a column of relative paths that will
be read by the command for you when using the -I/--input-dir flag:

Scraping a HTML column:

    $ xan scrape title document docs.csv > docs-with-titles.csv

Scraping HTML files on disk, using the -I/--input-dir flag:

    $ xan scrape title path -I ./docs > docs-with-title.csv

Then, this command knows how to scrape typical stuff from HTML such
as titles, urls, images and other metadata using very optimized routines
or can let you define a custom scraper that you can give through
the -e/--evaluate or -f/--evaluate-file.

The command can of course use multiple CPUs to go faster using -p/--parallel
or -t/--threads.

# Builtin scrapers

    - "title": scrape the content of the <title> tag if any
    - "canonical": scrape the canonical link if any
    - "urls": find all urls linked in the document

# Custom scrapers

When using -e/--evaluate or -f/--evaluate-file, this command is able to
leverage a custom CSS-like language to describe exactly what you want to
scrape.

Given scraper will either run once per HTML document or one time per
element matching the CSS selector given to -F/--foreach.

Example scraping the first h2 title from each document:

    $ xan scrape -e 'h2 > a {title: text; url: attr("href");}' html docs.csv

Example scraping all the h2 title from each document:

    $ xan scrape --foreach 'h2 > a' -e '& {title: text; url: attr("href");}' html docs.csv

A full reference of this language can be found using `xan help scraping`.

# Singular or plural?

Scrapers can be "singular" or "plural".

A singular scraper will produce exactly one output row per input row,
while a plural scraper can produce 0 to n output rows per input row.

Singular builtin scrapers: "title", "canonical".

Plural builtin scrapers: "urls".

Custom scrapers are singular, except when using -F/--foreach.

It can be useful, especially when using plural scrapers, to use
the -k/--keep flag to select the input columns to keep in the output.

Usage:
    xan scrape -e <expr> <column> [options] [<input>]
    xan scrape -f <path> <column> [options] [<input>]
    xan scrape title <column> [options] [<input>]
    xan scrape canonical <column> [options] [<input>]
    xan scrape urls <column> [options] [<input>]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>       If given, evaluate the given scraping expression.
    -f, --evaluate-file <path>  If given, evaluate the scraping expression found
                                in file at <path>.
    -I, --input-dir <path>      If given, target column will be understood
                                as relative path to read from this input
                                directory instead.
    -k, --keep <column>         Selection of columns from the input to keep in
                                the output.
    -p, --parallel              Whether to use parallelization to speed up computations.
                                Will automatically select a suitable number of threads to use
                                based on your number of cores. Use -t, --threads if you want to
                                indicate the number of threads yourself.
    -t, --threads <threads>     Parellize computations using this many threads. Use -p, --parallel
                                if you want the number of threads to be automatically chosen instead.

scrape url/links options:
    -u, --url-column <column>  Column containing the base url for given HTML.

scrape -e/--evaluate & -f/--evaluate-file options:
    -F, --foreach <css>  If given, will return one row per element matching
                         the CSS selector in target document, instead of returning
                         a single row per document.
    --sep <char>            Separator to use when serializing lists.
                         [default: |]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
