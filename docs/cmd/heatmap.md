<!-- Generated -->
# xan heatmap

```txt
Draw a heatmap from CSV data.

Use the --show-gradients flag to display a showcase of available
color gradients.

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --show-gradients
    xan heatmap --green-hills
    xan heatmap --help

heatmap options:
    -G, --gradient <name>  Gradient to use. Use --show-gradients to see what is
                           available.
                           [default: or_rd]
    -m, --min <n>          Minimum value for a cell in the heatmap. Will clamp
                           irrelevant values and use this min for normalization.
    -M, --max <n>          Maximum value for a cell in the heatmap. Will clamp
                           irrelevant values and use this max for normalization.
    --normalize <mode>     How to normalize the heatmap's values. Can be one of
                           "full", "row" or "col".
                           [default: full]
    -S, --size <n>         Size of the heatmap square in terminal rows.
                           [default: 1]
    -D, --diverging        Use a diverging color gradient. Currently only shorthand
                           for "--gradient rd_bu".
    -C, --cram             Attempt to cram column labels over the columns.
                           Usually works better when -S, --scale > 1.
    -N, --show-numbers     Whether to attempt to show numbers in the cells.
                           Usually only useful when -S, --scale > 1.
    --color <when>         When to color the output using ANSI escape codes.
                           Use `auto` for automatic detection, `never` to
                           disable colors completely and `always` to force
                           colors, even when the output could not handle them.
                           [default: auto]
    --repeat-headers <n>   Repeat headers every <n> heatmap rows. This can also
                           be set to "auto" to choose a suitable number based
                           on the height of your terminal.
    --show-gradients       Display a showcase of available gradients.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
