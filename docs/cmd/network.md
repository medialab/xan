<!-- Generated -->
# xan network

```txt
Process CSV data to build a network (nodes & edges) so you can produce a variety
of output ranging from graph data formats (e.g. json or gexf) or other CSV
outputs that can be useful to interpret network information easily when piped
into other xan commands.

Supported input modes:
    `edgelist`:  converts a CSV of edges with a column representing
                 sources and another column targets.
    `bipartite`: converts a CSV with two columns representing the
                 edges between both parts of a bipartite graph.

Supported output formats (-f, --format):
    `json`     - Graphology JSON serialization format
                 ref: https://graphology.github.io/serialization.html
    `gexf`     - Graph eXchange XML Format
                 ref: https://gexf.net/
    `nodelist` - CSV nodelist, with optional degrees if using -D/--degrees
    `stats`    - Single CSV row of useful graph statistics (number of nodes, edges,
                 graph type, density etc.)

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network bipartite [options] <part1> <part2> [<input>]
    xan network --help

output format options:
    -f, --format <format>     One of "json", "gexf", "stats" or "nodelist".
                              [default: json]
    --gexf-version <version>  GEXF version to output. Can be one of "1.2"
                              or "1.3".
                              [default: 1.2]

xan network options:
    -L, --largest-component   Only keep the largest connected component
                              in the resulting graph.
    -S, --simple              Use to indicate you know beforehand that processed
                              graph is simple, i.e. it does not contains multiple
                              edges for a same (source, target) pair. This can
                              improve performance of the overall process.
    -D, --degrees             Whether to compute node degrees so it can be added
                              to relevant outputs. Currently only relevant
                              when using -f "nodelist".

edgelist options:
    -U, --undirected       Whether the graph is undirected.
    --nodes <path>         Path to a CSV file containing node metadata
                           (use "-" to feed the file from stdin).
    --node-column <name>   Name of the column containing node keys.
                           [default: node]

bipartite options:
    --disjoint-keys  Pass this if you know both partitions of the graph
                         use disjoint sets of keys (i.e. if you know they share
                         no common keys at all). Incorrect graphs will be produced
                         if some keys are used by both partitions!

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
```
