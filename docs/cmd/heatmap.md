<!-- Generated -->
# xan heatmap

```txt
Draw a heatmap from CSV data.

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --help

heatmap options:
    -m, --min <n>       Minimum value for a cell in the heatmap. Will clamp
                        irrelevant values and use this min for normalization.
    -M, --max <n>       Maximum value for a cell in the heatmap. Will clamp
                        irrelevant values and use this max for normalization.
    -S, --scale <n>     Size of the heatmap square in terminal rows.
                        [default: 1]
    -D, --diverging     Use a diverging color gradient.
    -C, --force-colors  Force colors even if output is not supposed to be able to
                        handle them.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
