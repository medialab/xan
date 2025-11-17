<!-- Generated -->
# xan bins

```txt
Discretize selection of columns containing continuous data into bins.

The resulting bins table will be formatted thusly:

field       - Name of the column
value       - Bin's label (depends on what was given to -l/--label)
lower_bound - Lower bound of the bin
upper_bound - Upper bound of the bin
count       - Number of rows falling into this bin

The number of bins can be chosen with the -b/--bins flag. Note that,
by default, this number is an approximate goal since the command
attempts to find readble boundaries for the bins and this make it
hard to respect a precise number of bins. Use the -e/--exact flag
if you want to force the command to respect -b/--bins exactly.

Combined with `xan hist`, this command can be very useful to visualize
distributions of continous columns:

    $ xan bins -s count data.csv | xan hist

Using a log scale:

    $ xan bins -s count data.csv | xan hist --scale log

Usage:
    xan bins [options] [<input>]
    xan bins --help

bins options:
    -s, --select <arg>      Select a subset of columns to compute bins for. See
                            'xan select --help' for more detail.
    -b, --bins <number>     Number of bins to generate. Note that without -e/--exact,
                            this number should be considered as an approximate goal.
                            The command by default attempts to find nice & readable boundaries
                            for the bins and this means a precise number of bins is not
                            always achievable.
                            [default: 10]
    -H, --heuristic <name>  Heuristic to use to automatically find an adequate number
                            of bins. Must be one of `freedman-diaconis`, `sqrt` or `sturges`.
    --max-bins <number>     Maximum number of bins to generate. Only useful when using
                            the -H/--heuristic flag.
    -e, --exact             Whether to make sure to return the exact number of bins
                            provided to -b/--bins, which means the readability of the
                            bins boundaries might suffer.
    -l, --label <mode>      Label to choose for the bins (that will be placed in the
                            `value` column). Mostly useful to tweak representation when
                            piping to `xan hist`. Can be one of "full", "lower" or "upper".
                            [default: full]
    -m, --min <min>         Override min value. Values lower that this min will be counted
                            as out of bounds.
    -M, --max <max>         Override max value. Values greater that this max will be counted
                            as out of bounds.
    -N, --no-extra          Don't include, empty cells, nans and out of bounds counts.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
