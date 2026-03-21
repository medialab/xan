<!-- Generated -->
# xan heatmap

```txt
Render CSV data as a heatmap grid. x-axis labels will be taken from file's headers
(or 0-based column indices when used with -n/--no-headers). While y-axis labels
will be taken from the file's first column by default. All columns beyond the
first one will be considered as numerical and used to draw the heatmap grid.

If your file is not organized thusly, you can still use the -l/--label flag
to select the y-axis label column and/or the -v/--values flag to select columns
to be considered to draw the heatmap grid.

This command is typically used to display the results of `xan matrix`. For instance,
here is how to draw a correlation matrix:

    $ xan matrix corr -s 'sepal_*,petal_*' iris.csv | xan heatmap --diverging --unit

Here is another example drawing an adjacency matrix:

    $ xan matrix adj source target edges.csv | xan heatmap

Note that drawn matrices do not have to be square and can really be anything.
It is possible to think of the result as the symbolic representation of given
tabular data where each cell is represented by a square with a continuous color.

Consider the following example, for instance, where we draw a heatmap of Twitter
account popularity profiles wrt retweets, replies and likes:

    $ xan groupby user_screen_name \
    $   'mean(retweet_count) as rt, mean(reply_count) as rp, mean(like_count) as lk' \
    $   tweets.csv | \
    $ xan heatmap --size 2 --cram --show-numbers

You can also achieve a result similar to conditional formatting in a spreadsheet
by leveraging the -w/--width flag and showing numbers thusly:

    $ xan matrix count lang1 lang2 data.csv | xan heatmap -w 6 --show-numbers

Note that, by default, since there is not enough place on the x-axis, labels will be
printed in a legend before the heatmap itself. If you can afford the space, feel
free to use a -S/--size greater then 1 and toggle the -C/--cram flag to fit the
labels on top of the x-axis instead.

Increasing -S/--size also means you can try fitting the numbers within the heatmap's
cells themselves using -N/--show-numbers.

Finally, if you want a showcase of available color gradients, use the --show-gradients
flag.

Usage:
    xan heatmap [options] [<input>]
    xan heatmap --show-gradients
    xan heatmap --green-hills
    xan heatmap --help

heatmap options:
    -l, --label <column>    Column containing the y-axis labels. Defaults to
                            the first column of the file.
    -v, --values <columns>  Columns containing numerical values to display in the
                            heatmap. Defaults to all columns of the file beyond
                            the first one.
    -G, --gradient <name>   Gradient to use. Use --show-gradients to see what is
                            available.
                            [default: or_rd]
    -m, --min <n>           Minimum value for a cell in the heatmap. Will clamp
                            irrelevant values and use this min for normalization.
    -M, --max <n>           Maximum value for a cell in the heatmap. Will clamp
                            irrelevant values and use this max for normalization.
    -U, --unit              Shorthand for --min 0, --max 1 or --min -1, --max 1 when
                            using -D/--diverging.
    --normalize <mode>      How to normalize the heatmap's values. Can be one of
                            "full", "row" or "col".
                            [default: full]
    -S, --size <n>          Size of the heatmap square in terminal rows.
                            [default: 1]
    -w, --width <n>         Use this to set heatmap grid cells width if you want
                            rectangles instead of squares and want to have more
                            space to display cell numbers with -N/--show-numbers
                            or -Z/--show-normalized.
    -D, --diverging         Use a diverging color gradient. Currently only shorthand
                            for "--gradient rd_bu".
    -C, --cram              Attempt to cram column labels over the columns.
                            Usually works better when -S/--size > 1.
    -N, --show-numbers      Whether to attempt to show numbers in the cells.
                            Usually only useful when -S/--size > 1.
                            Cannot be used with -Z/--show-normalized.
    -Z, --show-normalized   Whether to attempt to show normalized numbers in the
                            cells. Usually only useful when -S/--size > 1.
                            Cannot be used with -N/--show-numbers.
    --color <when>          When to color the output using ANSI escape codes.
                            Use `auto` for automatic detection, `never` to
                            disable colors completely and `always` to force
                            colors, even when the output could not handle them.
                            [default: auto]
    --repeat-headers <n>    Repeat headers every <n> heatmap rows. This can also
                            be set to "auto" to choose a suitable number based
                            on the height of your terminal.
    --show-gradients        Display a showcase of available gradients.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
