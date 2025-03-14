<!-- Generated -->
# xan help

```txt
Print help about the `xan` expression language.

`xan help cheatsheet` will print a short cheatsheet about
how the language works. It can also be found online here:
https://github.com/medialab/xan/blob/master/docs/moonblade/cheatsheet.md

`xan help functions` will print the reference of all of the language's
functions (used in `xan select -e`, `xan map`, `xan filter`, `xan transform`,
`xan flatmap` etc.). It can also be found online here:
https://github.com/medialab/xan/blob/master/docs/moonblade/functions.md

`xan help aggs` will print the reference of all of the language's
aggregation functions (as used in `xan agg` and `xan groupby` mostly).
It can also be found online here:
https://github.com/medialab/xan/blob/master/docs/moonblade/aggs.md

`xan help scraping` will print information about the DSL used
by `xan scrape` and the related functions. It can also be found online here:
https://github.com/medialab/xan/blob/master/docs/moonblade/scraping.md

Use the -p/--pager flag to open desired documentation in a suitable
pager.

Use the -O/--open to read the desired documentation online (might
be slightly out of date!).

Usage:
    xan help cheatsheet [options]
    xan help functions [options]
    xan help aggs [options]
    xan help scraping [options]
    xan help --help

help options:
    -O, --open             Open the desired docs in a web browser.
    -p, --pager            Pipe the help into a pager (Same as piping
                           with forced colors into `less -SRi`).
    -S, --section <query>  Filter the `functions` doc to only include
                           sections matching the given case-insensitive
                           query.
    --json                 Dump the help as JSON data.
    --md                   Dump the help as Markdown.

Common options:
    -h, --help             Display this message
```
