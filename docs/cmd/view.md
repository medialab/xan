<!-- Generated -->
# xan view

```txt
Preview CSV data in the terminal in a human-friendly way with aligned columns,
shiny colors & all.

The command will by default try to display as many columns as possible but
will truncate cells/columns to avoid overflowing available terminal screen.

If you want to display all the columns using a pager, prefer using
the -p/--pager flag that internally rely on the ubiquitous "less"
command.

If you still want to use a pager manually, don't forget to use
the -e/--expand and -C/--force-colors flags before piping like so:

    $ xan view -eC file.csv | less -SR

Finally, it is possible to customize the default behavior of this command through
the "XAN_VIEW_ARGS" environment variable. This variable takes a series of
supported flags: -t/--theme, -p/--pager, -l/--limit, -R/--rainbow, -E/--sanitize-emojis,
and -S/--significance, -I/--hide-index & -M/--hide-info.

So if you want, for instance, to use the borderles theme, hide the index column and
restrict the number of floating points decimals to be shown by default:

    $ XAN_VIEW_ARGS="-t borderless -S 5 -I"

Usage:
    xan view [options] [<input>]
    xan v [options] [<input>]
    xan view --help

view options:
    -s, --select <arg>      Select the columns to visualize. See 'xan select -h'
                            for the full syntax.
    -t, --theme <name>      Theme for the table display, one of: "table", "borderless",
                            "compact", "rounded", "slim" or "striped".
                            [default: table]
    -p, --pager             Automatically use the "less" command to page the results.
                            This flag does not work on windows!
    -A, --all               Remove the row limit and display everything.
    -l, --limit <number>    Maximum of rows to read into memory. Use -A, --all or
                            set to 0 to disable the limit.
                            [default: 100]
    -R, --rainbow           Alternating colors for columns, rather than color by value type.
    --cols <num>            Width of the graph in terminal columns, i.e. characters.
                            Defaults to using all your terminal's width or 80 if
                            terminal's size cannot be found (i.e. when piping to file).
                            Can also be given as a ratio of the terminal's width e.g. "0.5".
    -C, --force-colors      Force colors even if output is not supposed to be able to
                            handle them.
    -e, --expand            Expand the table so that in can be easily piped to
                            a pager such as "less", with larger width constraints.
    -E, --sanitize-emojis   Replace emojis by their shortcode to avoid formatting issues.
    -S, --significance <n>  Maximum floating point significance used to format numbers.
    -I, --hide-index        Hide the row index on the left.
    -H, --hide-headers      Hide the headers. Implied when -n, --no-headers is given.
    -M, --hide-info         Hide information about number of displayed columns, rows etc.
    -g, --groupby <cols>    Isolate and emphasize groups of rows, represented by consecutive
                            rows with identical values in selected columns.
    -r, --right <col>       Force right alignment of selected columns.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not considered as being
                           the file header.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
