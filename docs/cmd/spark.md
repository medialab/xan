<!-- Generated -->
# xan spark

```txt
Print ASCII sparklines (using ▁▂▃▄▅▆▇ characters) from CSV data.

This command is able to represent arbitrary numerical series as well as
categorical data, time series, distributions etc.

Print two numerical columns:

    $ xan spark count1,count2 data.csv

    count1 ▅▆▅▇▅▃▅▅▁▅▅▄▅▄▆▃▅▄▃▁
    count2 ▆▆▅▇▅▃▅▅▁▆▆▄▅▄▅▃▄▅▄▁

Print the distribution of two numerical columns:

    $ xan spark count1,count2 -D -H 2 -z data.csv

           ▇▃
    count1 ██▇▆▆▂▁▃▁▁ ▁▂▃▁▃▁▂
           ▇▂
    count2 ██▇▆▆▆▆▁▄ ▂ ▂▃▂▂▁▂

Print time series grouped by the value of a column:

    $ xan spark value -T date -g group data.csv

    group1    ▁▂▃▄▆▇▇▅▃▂▁▁▁
    group2           ▂▇▇▇▄▁
    group3 ▁▂▃▇▆▆▆▄▂▁▁▁
    group4      ▂▄▅▇▄▆▃▂▂▂▁
    group5           ▁▂▂▄▇▄
    group6 ▅▇▇▆▃▁▁▁▁▁▁▁▁▁▂▁

Print a vertical bar chart from the output of `xan freq`:

    $ xan freq -s category data.csv | xan spark count -c value -H .5 -W7 -C always --hide-names -N

    ▇▇▇▇▇▇▃▃▃▃▃▃
    ████████████▆▆▆▆▆▆▂▂▂▂▂▂▁▁▁▁▁▁
    ██████████████████████████████▁▁▁▁▁▁
    94    85    75    66    64    48
    Vinyl  Disc Other Downl… Tape Strea…

Print categorical bar chart by group:

    $ xan spark value -c category -g author_name

    group1 ▇▁▁
    group1  ▇
    group1    ▇
    group1 ▂  ▇
    group1 ▇ ▁

Print the very literal Joy Division plot from the "Unknown Pleasures" album cover:

    $ curl https://gist.githubusercontent.com/borgar/31c1e476b8e92a11d7e9/raw/0fae97dab6830ecee185a63c1cee0008f6778ff6/pulsar.csv | \
    $ xan spark --along-rows '*' --hide-all

Unzoom your terminal for better effect ;)

Usage:
    xan spark debate
    xan spark --count [options] [<input>]
    xan spark [options] [--] <y> [<input>]
    xan spark --help

spark mode options:
    --along-rows          Collect series to print along rows, instead of along
                          columns.
    -T, --time <col>      Use selected <col> to position points in time and
                          reorder x axis chronologically.
    -D, --dist            Reinterpret given series by printing an histogram
                          of their distribution instead.
    -g, --groupby <cols>  Print one series per value found in <cols> selection.
    -c, --category <col>  Choose a <col> to represent a data point's category
                          in printed series. Will be used to select a color
                          for the data point and is therefore incompatible with
                          the other coloring flags below.

spark options:
    -W, --width <n>     Number of characters wide a sparkline bar is allowed to be.
                        [default: 1]
    -H, --height <n>    Number of characters high a sparkline bar is allowed to be.
                        Can also be given as a ratio or percentage of the terminal's
                        height e.g. "45%" or "0.5". Defaults to 1.
    --scale <scale>     Apply a scale to the y axis. Can be one of "lin", "pow",
                        "sqrt", "pow(custom_exponent)" like "pow(4.5)", "log",
                        "log2", "log10" or "log(custom_base)" like "log(2.5)".
                        [default: lin]
    --log               Use a log scale, shorthand for --scale=log.
    -m, --min <n>       Force <y> minimum value. Any value falling out of range will be
                        filtered out.
    -M, --max <n>       Force <y> maximum value. Any value falling out of range will be
                        filtered out.
    --share-scale       Whether to force series to share their y-axis.
    --hide-names        Whether to hide series' names.
    --hide-legend       Whether to hide any kind of legend.
    --hide-all          Shorthand for --hide-names, --hide-legend.
    -F, --flatter       Print series names on top of them instead of to their left, to
                        make more space for series themselves.
    -w, --wrap          Allow series to overflow on muliple lines instead of discretizing
                        them to fit your terminal's width.
    -S, --small-multiples <n>
                        When used, will display <n> series per row instead of a single one.
                        This is useful to see more series at once if you have enough space.
    -N, --show-numbers  Show series numbers under their respective bars. Only useful
                        when -W/--width is more than 1.
    -P, --show-percentages
                        Show series numbers as a percentage under their respective bars. Only
                        useful when -W/--width is more than 1.
    --repeat-x-axis <choice>
                        Whether to repeat x-axis for each plot when using -T/--time. Can be
                        "yes" or "no". [default: yes]
    --cols <num>        Number of terminal columns, i.e. characters, that we can
                        use for drawing labels, legends and sparklines.
                        Defaults to using all your terminal's width or 80 if
                        terminal size cannot be found (i.e. when piping to file).
                        Can also be given as a ratio or percentage of the terminal's width
                        e.g. "45%" or "0.5".
    --color <when>      When to color the output using ANSI escape codes.
                        Use `auto` for automatic detection, `never` to
                        disable colors completely and `always` to force
                        colors, even when the output could not handle them.
                        [default: auto]

spark coloring options:
    -G, --gradient <name>             Color each bar using given gradient.
    -B, --background-gradient <name>  Hide bars and print a background color using
                                      given gradient. The result can be thought of as a kind
                                      of heatmap.
    -V, --vertical-gradient <name>    Color bars with given gradient but map the color
                                      on a character's height in a bar. Use this for
                                      aesthetic purposes only. This is only ever useful
                                      when -H/--height is more than 1.
    -R, --rainbow                     Assign a color to each series following a cyclical pattern.
                                      This is useful to distinguish between series when using
                                      multiple <y> columns or -g/--groupby.
    -z, --striped                     Dim each odd bar's color for better readability.

See `xan help gradients` for a list of available gradients.

spark -T/--time options:
    --count                 Count rows falling into a same temporal bucket instead of
                            relying on a numerical column.
    --sort                  Sort given time series by starting point.
    -A, --aggregate <mode>  How to aggregate values falling into a same bucket when discretizing
                            a temporal x axis, e.g. when using the -T/--time flag.
                            Can be one of "sum" or "mean". Defaults to "sum" when --count
                            is given, else "mean".

spark -D/--dist options:
    -b, --bins <n>  Number of bins for the distribution histogram. [default: 35]

spark -c/--category options:
    -C, --cram <choice>  When printing a categorical legend, whether to attempt
                         cramming category names under a series' bars. Can be
                         either "always", "never" or "auto". When "auto" is given,
                         names will be crammed if theire is enough place for them.
                         This is really only useful when -W/--width is more than 1.
                         [default: auto]

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not be included in
                           the count.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
