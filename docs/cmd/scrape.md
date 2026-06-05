<!-- Generated -->
# xan scrape

```txt
Scrape HTML files (or any kind of XML files, really) and return structured
tabular data from the result.

This command can process a variety of different sources:

Paths to HTML documents on disk, by default
    $ xan scrape head page1.html page2.html page3.html > result.csv
    $ xan scrape head **/*.html > result.csv

Paths to HTML documents collected using a glob pattern, using --glob
    $ xan scrape head --glob '*.csv' > result.csv

A text file containing a HTML document path per line, using --paths
    $ xan scrape head --paths paths.txt > result.csv

A CSV file containing a column with HTML document paths, using --paths & --path-column
    $ xan scrape head --paths path.csv --path-column path > result.csv

A CSV file containing a column of inline HTML documents, using --docs & --doc-column
    $ xan scrape head --docs documents.csv --doc-column html > result.csv

A single HTML document fed through stdin, using -D/--stdin-doc
    $ curl -L https://www.lemonde.fr/ | xan scrape head -D > result.csv

Now regarding what we can scrape, th command knows how to extract typical stuff
from HTML documents such as titles, urls and other metadata using very optimized
routines.

Or you remain free to define a custom scraper that you can give through
the -e/--evaluate or -f/--evaluate-file flags.

Know also that this command is able to use multiple CPUs to go faster using -p/--parallel
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

Example scraping the first h2 title from a HTML document:

    $ xan scrape -e 'h2 > a {title: text; url: attr("href");}' page.html

Example scraping all the h2 titles from a HTML document:

    $ xan scrape --foreach 'h2 > a' -e '& {title: text; url: attr("href");}' page.html

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
    xan scrape (head|urls|article|images) [options] [<inputs>...]
    xan scrape -e <expr> [options] [<inputs>...]
    xan scrape -f <path> [options] [<inputs>...]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>       If given, evaluate the given scraping expression.
    -f, --evaluate-file <path>  If given, evaluate the scraping expression found
                                in file at <path>.
    --paths <input>             If given, reads <input> and consider it as containing
                                one document path per line.
    --path-column <name>        If given with --paths, consider <input> as a CSV file
                                instead and read document paths from selected column.
    --docs <input>              If givens, reads <input> and consider it as a CSV
                                file with a column containing inline documents.
                                Requires --doc-column to be given.
    --doc-column <name>         Selects column containing inline documents given
                                through --docs.
    -D, --stdin-doc             When set, the command will read the content of stdin as
                                a single document. This can be useful when piping the
                                result of `curl` or `wget` into the command directly.
    --glob <pattern>            If given, collects document paths to process by applying
                                the given glob pattern.
    -E, --encoding <name>       Encoding of to read on disk. Will default utf-8.
    -k, --keep <column>         Selection of columns from the input to keep in
                                the output. Default is to keep all columns from input.
    -I, --input-dir <path>      When set, processed paths will be read relative to the
                                given base <path>.
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

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
