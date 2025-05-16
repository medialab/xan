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
    -§, --cursed           Return a c̵̱̝͆̓ṳ̷̔r̶̡͇͓̍̇š̷̠̎e̶̜̝̿́d̸͔̈́̀ sample from a Lovecraftian kinda-uniform
                           distribution (source: trust me), without requiring to read
                           the whole file. Instead, we will randomly jump through it
                           like a dark wizard. This means the sampled file must
                           be large enough and seekable, so no stdin nor gzipped files.
                           Rows at the very end of the file might be discriminated against
                           because they are not cool enough. If desired sample size is
                           deemed too large for the estimated total number of rows, the
                           c̵̱̝͆̓ṳ̷̔r̶̡͇͓̍̇š̷̠̎e̶̜̝̿́d̸͔̈́̀  routine will fallback to normal reservoir sampling to
                           sidestep the pain of learning O(∞) is actually a thing.
                           Does not work with -w/--weight nor -g/--groupby.

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
