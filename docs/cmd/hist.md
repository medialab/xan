<!-- Generated -->
# xan hist

```txt
Print a horizontal histogram for the given CSV file with each line
representing a bar in the resulting graph.

This command is very useful when used in conjunction with the `frequency` or `bins`
command.

Usage:
    xan hist [options] [<input>]
    xan hist --help

hist options:
    --name <name>            Name of the represented field when no field column is
                             present. [default: unknown].
    -f, --field <name>       Name of the field column. I.e. the one containing
                             the represented value (remember this command can
                             print several histograms). [default: field].
    -l, --label <name>       Name of the label column. I.e. the one containing the
                             label for a single bar of the histogram. [default: value].
    -v, --value <name>       Name of the count column. I.e. the one containing the value
                             for each bar. [default: count].
    -S, --simple             Use simple characters to display the bars that will be less
                             detailed but better suited to be written as raw text.
    --cols <num>             Width of the graph in terminal columns, i.e. characters.
                             Defaults to using all your terminal's width or 80 if
                             terminal's size cannot be found (i.e. when piping to file).
                             Can also be given as a ratio of the terminal's width e.g. "0.5".
    -R, --rainbow            Alternating colors for the bars.
    -m, --domain-max <type>  If "max" max bar length will be scaled to the
                             max bar value. If "sum", max bar length will be scaled to
                             the sum of bar values (i.e. sum of bar lengths will be 100%).
                             Can also be an absolute numerical value, to clamp the bars
                             or make sure different histograms are represented using the
                             same scale.
                             [default: max]
    -C, --force-colors       Force colors even if output is not supposed to be able to
                             handle them.
    -P, --hide-percent       Don't show percentages.
    -u, --unit <unit>        Value unit.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```