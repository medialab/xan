<!-- Generated -->
# xan sample

```txt
Randomly samples CSV data uniformly using memory proportional to the size of
the sample.

This command is intended to provide a means to sample from a CSV data set that
is too big to fit into memory (for example, for use with commands like 'xan freq'
or 'xan stats'). It will however visit every CSV record exactly
once, which is necessary to provide a uniform random sample. If you wish to
limit the number of records visited, use the 'xan slice' command to pipe into
'xan sample'.

The command can also extract a biased sample based on a numeric column representing
row weights, using the --weight flag.

Usage:
    xan sample [options] <sample-size> [<input>]
    xan sample --help

sample options:
    --seed <number>        RNG seed.
    -w, --weight <column>  Column containing weights to bias the sample.
    -g, --groupby <cols>   Return a sample per group.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will be consider as part of
                           the population to sample from. (When not set, the
                           first row is the header row and will always appear
                           in the output.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
```
