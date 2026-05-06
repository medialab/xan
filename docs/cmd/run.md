<!-- Generated -->
# xan run

```txt
Run the given xan pipeline or execute a xan script.

Example:

    $ xan run 'search -s category tape | count' data.csv

# Script files

This command is also able to run a script written in a file like so:

*script.xan*

```
# This can include comments
search -s Category -e Tape |
count
```

    $ xan run -f script.xan data.csv

Note that in script files you can omit `xan` before the commands (or you can
also keep it, it does not matter). You can also have comments starting with `#`.

The syntax of those scripts can be thought of as POSIX shell and it will be parsed
first by normalizing CRLF newlines to LF then using `shlex`.

Note that to make sure your script is compatible across different OSes you should
favor using `/` (forward slashes) in paths, since most modern Windows shells
know how to handle both slashes and backslashes in paths and no normalization
of paths will be done by this command.

# Regarding input

If you don't give an <input> path to this command, the first command of given
pipeline will be fed the same stdin as what was given to the `xan run` call.

This ultimately means you can very well hardcode the input path of the pipeline's
first command within the script if you wish to.

If you do give an <input> path, it will be forwarded as last argument to the
first command of the pipeline.

Usage:
    xan run [options] <pipeline> [<input>]
    xan run --help

run options:
    -f, --file  Run <pipeline> from a script file instead.
    -T, --tee   Interleave a call to `xan view -T` between each step of given
                pipeline, hence printing a short view of each transitive
                step. Will not work with non-CSV inputs nor with hardcoded
                paths in first command of the pipeline.

Common options:
    -h, --help             Display this message
```
