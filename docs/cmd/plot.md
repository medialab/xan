<!-- Generated -->
# xan plot

```txt
Draw a scatter plot or a line plot based on 2-dimensional data.

It is also possible to draw multiple series/lines, as well as drawing multiple
series/lines as small multiples (sometimes also called a facet grid), by providing
a -c/--category column or selecting multiple columns as <y> series.

This command is also able to draw a temporal x axis when given the -T/--time flag
and accepts the following formats:

* A full ISO datetime or Z-terminated timestamp
* A standard timestamp in seconds
* A full or partial ISO date (e.g. 2025-03-12, 2025-03, 2025)

Use `xan map`, `xan select -e` or `xan transform` to deal with other datetime
formats ahead of the `xan plot` command.

Drawing a simple scatter plot:

    $ xan plot sepal_width sepal_length iris.csv

Drawing a categorical scatter plot:

    $ xan plot sepal_width sepal_length -c species iris.csv

The same, as small multiples:

    $ xan plot sepal_width sepal_length -c species iris.csv -S 2

As a line chart:

    $ xan plot -L sepal_length petal_length iris.csv

Plotting time series:

    $ xan plot -LT datetime units sales.csv

Plotting millisecond timestamps time series:

    $ xan select -e 'timestamp_ms(time)' | xan plot -LT 0 --count

Plotting multiple comparable times series at once:

    $ xan plot -LT datetime amount,amount_fixed sales.csv

Different times series, as small multiples:

    $ xan plot -LT datetime revenue,units sales.csv -S 2

Usage:
    xan plot --count [options] <x> [<input>]
    xan plot [options] <x> <y> [<input>]
    xan plot --help

plot options:
    -L, --line                 Whether to draw a line plot instead of the default scatter plot.
    -B, --bars                 Whether to draw bars instead of the default scatter plot.
                               WARNING: currently does not work if y range does not include 0.
                               https://github.com/ratatui/ratatui/issues/1391
    -T, --time                 Use to indicate that the x axis is temporal. The axis will be
                               discretized according to some inferred temporal granularity and
                               y values will be summed wrt the newly discretized x axis.
    --count                    Omit the y column and count rows instead. Only relevant when
                               used with -T, --time that will discretize the x axis.
    -A, --aggregate <expr>     Expression that will be used to aggregate values falling into
                               the same bucket when discretizing the x axis, e.g. when using
                               the -T, --time flag. The `_` implicit variable will be use to
                               denote a value in said expression. For instance, if you want
                               to average the values you can pass `mean(_)`. Will default
                               to `sum(_)`.
    -c, --category <col>       Name of the categorical column that will be used to
                               draw distinct series per category.
                               Does not work when selecting multiple columns with <y>.
    -R, --regression-line      Draw a regression line. Only works when drawing a scatter plot with
                               a single series.
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
    -S, --small-multiples <n>  Display small multiples (also called facet grids) of datasets
                               given by -c, --category or when multiple series are provided to <y>,
                               using the provided number of grid columns. The plot will all share the same
                               x scale but use a different y scale by default. See --share-y-scale
                               and --separate-x-scale to tweak this behavior.
    --share-x-scale <yes|no>   Give "yes" to share x scale for all plot when drawing small multiples with -S,
                               or "no" to keep them separate.
                               [default: yes]
    --share-y-scale <yes|no>   Give "yes" to share y scale for all plot when drawing small multiples with -S,
                               or "no" to keep them separate. Defaults to "yes" when -c, --category is given
                               and "no" when multiple series are provided to <y>.
    -M, --marker <name>        Marker to use. Can be one of (by order of size): 'braille', 'dot',
                               'halfblock', 'bar', 'block'.
                               [default: braille]
    -G, --grid                 Draw a background grid.
    --x-ticks <n>              Approx. number of x-axis graduation steps. Will default to some
                               sensible number based on the dimensions of the terminal.
    --y-ticks <n>              Approx. number of y-axis graduation steps. Will default to some
                               sensible number based on the dimensions of the terminal.
    --x-min <n>                Force a minimum value for the x axis.
    --x-max <n>                Force a maximum value for the x axis.
    --y-min <n>                Force a minimum value for the y axis.
    --y-max <n>                Force a maximum value for the y axis.
    --x-scale <scale>          Apply a scale to the x axis. Can be one of "lin", "log",
                               "log2", "log10" or "log(custom_base)" like "log(2.5)".
                               [default: lin]
    --y-scale <scale>          Apply a scale to the y axis. Can be one of "lin", "log",
                               "log2", "log10" or "log(custom_base)" like "log(2.5)".
                               [default: lin]
    --color <when>             When to color the output using ANSI escape codes.
                               Use `auto` for automatic detection, `never` to
                               disable colors completely and `always` to force
                               colors, even when the output could not handle them.
                               [default: auto]
    -i, --ignore               Ignore values that cannot be correctly parsed.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
```
