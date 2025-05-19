use crate::util;
use crate::CliResult;

static USAGE: &str = "
Print script parts necessary to activate xan completions, tailored
to your current shell.

Only support `bash` and `zsh` for now (or at least any shell relying on
`complete` and `compgen`).

For `bash`, run:
    $ xan completions bash >> ~/.bashrc

To enable those completions system-wide, you can also run:
    $ xan completions bash > /etc/bash_completions.d/xan

For `zsh`, run:
    $ xan completions zsh >> ~/.zshrc

For `zsh` you might also need to load Bash compatibility wrt completions thusly:

    $ echo 'autoload -Uz bashcompinit && bashcompinit' >> ~/.zshrc

You will need to reload you shell or source your main shell
configuration file (`source ~/,bashrc` for bash, for instance) for
the completions to be activated (this is only required once).

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
complete -C __xan -o default xan
";

static ZSH_COMPLETE_FUNCTION: &str = "
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

    if args.cmd_bash {
        println!("{}", BASH_COMPLETE_FUNCTION.trim_end());
    } else if args.cmd_zsh {
        println!("{}", ZSH_COMPLETE_FUNCTION.trim_end());
    }

    Ok(())
}
