use std::env;
use std::fs;
use std::io::{self, IsTerminal};
use std::process::{Command, Stdio};

use crate::processing::{parse_pipeline, Children};
use crate::util;
use crate::CliResult;

// TODO: -T, binary serialization, file with comments
// TODO: inherit for all stderr stream? no. proper error handling using a checker thread

static USAGE: &str = "
Run the given xan pipeline or execute a xan script.

Examples:

Running a simple pipeline:

    $ xan run 'search -s category tape | count' data.csv

Running a script file:

*script.xan*

```
# This can include comments
search -s Category -e Tape |
count
```

    $ xan run -f script.xan data.csv

Usage:
    xan run [options] <pipeline> [<input>]
    xan run --help

run options:
    -f, --file  Run <pipeline> from a script file instead.

Common options:
    -h, --help             Display this message
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_pipeline: String,
    arg_input: Option<String>,
    flag_file: bool,
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

    let pipeline = parse_pipeline(&args.arg_pipeline)?;
    let mut children = Children::with_capacity(pipeline.len());

    for (i, step) in pipeline.iter().enumerate() {
        let mut command = Command::new(exe.clone());

        if i + 1 == pipeline.len() {
            // Last item of the pipeline will write in stdout/stderr
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        } else {
            command.stdout(Stdio::piped()).stderr(Stdio::piped());
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
