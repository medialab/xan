<!-- Generated -->
# xan bins

```txt
Discretize selection of columns containing continuous data into bins.

The bins table is formatted as CSV data:

    field,value,lower_bound,upper_bound,count

Usage:
    xan bins [options] [<input>]
    xan bins --help

bins options:
    -s, --select <arg>     Select a subset of columns to compute bins
                           for. See 'xan select --help' for the format
                           details.
    -b, --bins <number>    Number of bins. Will default to using various heuristics
                           to find an optimal default number if not provided.
    -E, --nice             Whether to choose nice boundaries for the bins.
                           Might return a number of bins slightly different to
                           what was passed to -b/--bins, as a consequence.
    -l, --label <mode>     Label to choose for the bins (that will be placed in the
                           `value` column). Mostly useful to tweak representation when
                           piping to `xan hist`. Can be one of "full", "lower" or "upper".
                           [default: full]
    -m, --min <min>        Override min value.
    -M, --max <max>        Override max value.
    -N, --no-extra         Don't include, nulls, nans and out-of-bounds counts.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
