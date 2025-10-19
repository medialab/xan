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
    -B, --bar-size <size>    Size of the bar characters between "small", "medium"
                             and "large". [default: medium].
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
    -c, --category <col>     Name of the categorical column that will be used to
                             assign distinct colors per category.
                             Incompatible with -R, --rainbow.
    --color <when>           When to color the output using ANSI escape codes.
                             Use `auto` for automatic detection, `never` to
                             disable colors completely and `always` to force
                             colors, even when the output could not handle them.
                             [default: auto]
    -P, --hide-percent       Don't show percentages.
    -u, --unit <unit>        Value unit.
    -D, --dates              Set to indicate your values are dates (supporting year, year-month or
                             year-month-day). This will sort the bars by date, and add missing dates.
    -G, --compress-gaps <n>  If given, will compress gaps of minimum <n> consecutive
                             entries set to 0 and replace it with an ellipsis.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
