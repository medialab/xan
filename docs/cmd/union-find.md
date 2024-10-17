<!-- Generated -->
# xan union-find

```txt
Apply the union-find algorithm on a CSV file representing a graph's
edge list (one column for source nodes, one column for target nodes) in
order to return a CSV of nodes with a component label.

The command can also return only the nodes belonging to the largest connected
component using the -L/--largest flag or the sizes of all the connected
components of the graph using the -S/--sizes flag.

Usage:
    xan union-find <source> <target> [options] [<input>]
    xan union-find --help

union-find options:
    -L, --largest  Only return nodes belonging to the largest component.
                   The output CSV file will only contain a 'node' column in
                   this case.
    -S, --sizes    Return a single CSV column containing the sizes of the graph's
                   various connected components.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
