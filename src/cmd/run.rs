use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::process::{Command, Stdio};

use crate::processing::{parse_pipeline, Children};
use crate::util;
use crate::CliResult;

// TODO: binary serialization
// TODO: inherit for all stderr stream? no. proper error handling using a checker thread

static USAGE: &str = "
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
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_pipeline: String,
    arg_input: Option<String>,
    flag_file: bool,
    flag_tee: bool,
}

impl Args {
    fn resolve(&mut self) -> CliResult<()> {
        if self.flag_file {
            self.arg_pipeline = fs::read_to_string(&self.arg_pipeline)?;
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve()?;

    let exe = env::current_exe()?;

    let mut pipeline = parse_pipeline(&args.arg_pipeline)?;
    let mut children = Children::with_capacity(pipeline.len());

    if args.flag_tee {
        let mut interleaved_pipeline = Vec::with_capacity(pipeline.len() * 2);

        for (i, step) in pipeline.iter().enumerate() {
            let mut view_step = vec!["view".to_string(), "-T".to_string()];

            if i > 0 {
                view_step.push("--name".to_string());
                view_step
                    .push(shlex::try_join(pipeline[i - 1].iter().map(|s| s.as_str())).unwrap());
            }

            interleaved_pipeline.push(view_step);
            interleaved_pipeline.push(step.clone());
        }

        pipeline = interleaved_pipeline;
    }

    for (i, step) in pipeline.iter().enumerate() {
        let mut command = Command::new(exe.clone());

        if i + 1 == pipeline.len() {
            // Last item of the pipeline will write in stdout/stderr
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        } else {
            command.stdout(Stdio::piped()).stderr(Stdio::inherit());
        }

        for arg in step {
            command.arg(arg);
        }

        if let Some(last_child) = children.last_mut() {
            // Piping last command into the next
            command.stdin(
                last_child
                    .stdout
                    .take()
                    .expect("could not consume last child stdout"),
            );
        } else {
            // First command in pipeline must read the file
            if let Some(path) = &args.arg_input {
                command.stdin(Stdio::null());
                command.arg(path);
            } else {
                if io::stdin().is_terminal() {
                    Err("failed to read CSV data from stdin. Did you forget to give a path to your file?")?;
                }

                command.stdin(Stdio::inherit());
            }
        }

        children.push(command.spawn()?);
    }

    Ok(())
}
