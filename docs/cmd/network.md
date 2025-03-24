<!-- Generated -->
# xan network

```txt
Convert CSV data to graph data.

Supported input types:
    edgelist:  converts a CSV of edges with a column representing
               sources and another column targets.
    bipartite: converts a CSV with two columns representing the
               edges between both parts of a bipartite graph.

Supported output formats:
    json - Graphology JSON serialization format
           ref: https://graphology.github.io/serialization.html
    gexf - Graph eXchange XML Format
           ref: https://gexf.net/
    nodelist - CSV nodelist

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network bipartite [options] <part1> <part2> [<input>]
    xan network --help

xan network options:
    -f, --format <format>     One of "json", "gexf" or "nodelist".
                              [default: json]
    --gexf-version <version>  GEXF version to output. Can be one of "1.2"
                              or "1.3".
                              [default: 1.2]
    -L, --largest-component   Only keep the largest connected component
                              in the resulting graph.
    --stats                   Print useful statistics about the generated graph
                              in stderr.

network edgelist options:
    -U, --undirected       Whether the graph is undirected.
    --nodes <path>         Path to a CSV file containing node metadata
                           (use "-" to feed the file from stdin).
    --node-column <name>   Name of the column containing node keys.
                           [default: node]

network bipartite options:
    -D, --disjoint-keys  Pass this if you know both partitions of the graph
                         use disjoint sets of keys (i.e. if you know they share
                         no common keys at all). Incorrect graphs will be produced
                         if some keys are used by both partitions!

network -f "nodelist" options:
    --degrees  Whether to compute node degrees and add relevant columns to the
               CSV output.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
```
