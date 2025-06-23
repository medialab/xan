<!-- Generated -->
# xan scrape

```txt
Scrape HTML files to output tabular CSV data.

This command can either process a CSV file with a column containing
raw HTML, or a CSV file with a column of paths to read, relative to what is given
to the -I/--input-dir flag.

Scraping a HTML column:

    $ xan scrape head document docs.csv > enriched-docs.csv

Scraping HTML files on disk, using the -I/--input-dir flag:

    $ xan scrape head path -I ./downloaded docs.csv > enriched-docs.csv

Then, this command knows how to scrape typical stuff from HTML such
as titles, urls and other metadata using very optimized routines
or can let you define a custom scraper that you can give through
the -e/--evaluate or -f/--evaluate-file.

The command can of course use multiple CPUs to go faster using -p/--parallel
or -t/--threads.

# Builtin scrapers

Here is the list of `xan scrape` builtin scrapers along with the columns they
will add to the output:

"head": will scrape typical metadata found in <head> tags. Outputs one row
per input row with following columns:
    - title
    - canonical_url

"urls": will scrape all urls found in <a> tags in the document. Outputs one
row per scraped url per input row with following columns:
    - url

"images": will scrape all downloadable image urls found in <img> tags. Outputs
one row per scraped image per input row with following columns:
    - src

"article": will scrape typical news article metadata by analyzing the <head>
tag and JSON-LD data (note that you can combine this one with the -e/-f flags
to add custom data to the output, e.g. to scrape the article text). Outputs one
row per input row with the following columns:
    - canonical_url
    - headline
    - description
    - date_created
    - date_published
    - date_modified
    - section
    - keywords
    - authors
    - image
    - image_caption
    - free

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

# How many output rows per input row?

Scrapers can either output exactly one row per input row or 0 to n output rows
per input row.

Scrapers outputting exactly one row per input row: "head", "article", any
scraper given to -e/-f WITHOUT -F/--foreach.

Scrapers outputting 0 to n rows per input row: "urls", "images", any scraper
given to -e/-f WITH -F/--foreach.

It can be useful sometimes to use the -k/--keep flag to select the input columns
to keep in the output. Note that using this flag with an empty selection (-k '')
means outputting only the scraped columns.

Usage:
    xan scrape head <column> [options] [<input>]
    xan scrape urls <column> [options] [<input>]
    xan scrape article <column> [options] [<input>]
    xan scrape images <column> [options] [<input>]
    xan scrape -e <expr> <column> [options] [<input>]
    xan scrape -f <path> <column> [options] [<input>]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>       If given, evaluate the given scraping expression.
    -f, --evaluate-file <path>  If given, evaluate the scraping expression found
                                in file at <path>.
    -I, --input-dir <path>      If given, target column will be understood
                                as relative path to read from this input
                                directory instead.
    -E, --encoding <name>       Encoding of HTML to read on disk. Will default utf-8.
    -k, --keep <column>         Selection of columns from the input to keep in
                                the output. Default is to keep all columns from input.
    -p, --parallel              Whether to use parallelization to speed up computations.
                                Will automatically select a suitable number of threads to use
                                based on your number of cores. Use -t, --threads if you want to
                                indicate the number of threads yourself.
    -t, --threads <threads>     Parellize computations using this many threads. Use -p, --parallel
                                if you want the number of threads to be automatically chosen instead.

scrape url, links, images options:
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
