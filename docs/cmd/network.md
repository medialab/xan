<!-- Generated -->
# xan network

```txt
Convert CSV data to graph data.

Supported formats:
    json - Graphology JSON serialization format
           ref: https://graphology.github.io/serialization.html
    gexf - Graph eXchange XML Format
           ref: https://gexf.net/

Supported modes:
    edgelist: converts a CSV of edges with a column representing
              sources and another column targets.

Usage:
    xan network edgelist [options] <source> <target> [<input>]
    xan network --help

xan network options:
    -f, --format <format>     One of "json" or "gexf".
                              [default: json]
    --gexf-version <version>  GEXF version to output. Can be one of "1.2"
                              or "1.3".
                              [default: 1.2]
    -L, --largest-component   Only keep the largest connected component
                              in the resulting graph.

network edgelist options:
    -U, --undirected       Whether the graph is undirected.
    --nodes <path>         Path to a CSV file containing node metadata
                           (use "-" to feed the file from stdin).
    --node-column <name>   Name of the column containing node keys.
                           [default: node]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter foDirectedr reading CSV data.
                           Must be a single character.
```
