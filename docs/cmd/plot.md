<!-- Generated -->
# xan plot

```txt
Draw a scatter plot or a line plot based on 2-dimensional data.

Usage:
    xan plot --count [options] <x> [<input>]
    xan plot [options] <x> <y> [<input>]
    xan plot --help

plot options:
    -L, --line                 Whether to draw a line plot instead of the default scatter plot.
    -B, --bars                 Whether to draw bars instead of the default scatter plot.
                               WARNING: currently does not work if y range does not include 0.
                               (https://github.com/ratatui/ratatui/issues/1391)
    -T, --time                 Use to indicate that the x axis is temporal. The axis will be
                               discretized according to some inferred temporal granularity and
                               y values will be summed wrt the newly discretized x axis.
    --count                    Omit the y column and count rows instead. Only relevant when
                               used with -T, --time that will discretize the x axis.
    -C, --category <col>       Name of the categorical column that will be used to
                               draw distinct series per category.
                               Incompatible with -Y, --add-series.
    -Y, --add-series <col>     Name of another column of y values to add as a new series.
                               Incompatible with -C, --category.
    -g, --granularity <g>      Force temporal granularity for x axis discretization when
                               using -T, --time. Must be one of "years", "months", "days",
                               "hours", "minutes" or "seconds". Will be inferred if omitted.
    --cols <num>               Width of the graph in terminal columns, i.e. characters.
                               Defaults to using all your terminal's width or 80 if
                               terminal size cannot be found (i.e. when piping to file).
                               Can also be given as a ratio of the terminal's width e.g. "0.5".
    --rows <num>               Height of the graph in terminal rows, i.e. characters.
                               Defaults to using all your terminal's height minus 2 or 30 if
                               terminal size cannot be found (i.e. when piping to file).
                               Can also be given as a ratio of the terminal's height e.g. "0.5".
    -S, --small-multiples <n>  Display small multiples of datasets given by -C, --category
                               or -Y, --add-series using the provided number of grid columns.
                               The plot will all share the same x scale but use a different y scale by
                               default. See --share-y-scale and --separate-x-scale to tweak this behavior.
    --share-x-scale <yes|no>   Give "yes" to share x scale for all plot when drawing small multiples with -S,
                               or "no" to keep them separate.
                               [default: yes]
    --share-y-scale <yes|no>   Give "yes" to share y scale for all plot when drawing small multiples with -S,
                               or "no" to keep them separate. Defaults to "yes" when -C, --category is given
                               and "no" when -Y, --add-series is given.
    -M, --marker <name>        Marker to use. Can be one of (by order of size): 'braille', 'dot',
                               'halfblock', 'bar', 'block'.
                               [default: braille]
    -G, --grid                 Draw a background grid.
    --x-ticks <n>              Number of x-axis graduation steps. Will default to some sensible number based on
                               the dimensions of the terminal.
    --y-ticks <n>              Number of y-axis graduation steps. Will default to some sensible number based on
                               the dimensions of the terminal.
    --x-min <n>                Force a minimum value for the x axis.
    --x-max <n>                Force a maximum value for the x axis.
    --y-min <n>                Force a minimum value for the y axis.
    --y-max <n>                Force a maximum value for the y axis.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
```
