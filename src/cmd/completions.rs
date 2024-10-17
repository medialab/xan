use crate::util;
use crate::CliResult;

static USAGE: &str = "
Print script parts necessary to activate xan completions, tailored
to your current shell.

Only support `bash` and `zsh` for now (or at least any shell relying on
`complete` and `compgen`).

For `bash`, just run:
    $ xan completions bash >> ~/.bashrc

For `zsh`, just run:
    $ xan completions zsh >> ~/.zshrc

Usage:
    xan completions (bash | zsh)
    xan completions --help

Common options:
    -h, --help             Display this message
";

static BASH_COMPLETE_FUNCTION: &str = "
# Xan completions
function __xan {
    xan compgen \"$1\" \"$2\" \"$3\"
}
complete -F __xan -o default xan
";

#[derive(Deserialize)]
struct Args {
    cmd_bash: bool,
    cmd_zsh: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_bash || args.cmd_zsh {
        println!("{}", BASH_COMPLETE_FUNCTION.trim_end());
    }

    Ok(())
}
