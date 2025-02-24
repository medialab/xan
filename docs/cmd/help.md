<!-- Generated -->
# xan help

```txt
Print help about the `xan` expression language.

`xan help cheatsheet` will print a short cheatsheet about
how the language works.

`xan help functions` will print the reference of all of the language's
functions (used in `xan select -e`, `xan map`, `xan filter`, `xan transform`,
`xan flatmap` etc.).

`xan help aggs` will print the reference of all of the language's
aggregation functions (as used in `xan agg` and `xan groupby` mostly).

Usage:
    xan help cheatsheet [options]
    xan help functions [options]
    xan help aggs [options]
    xan help --help

help options:
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
