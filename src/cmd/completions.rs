use crate::util;
use crate::CliResult;

static USAGE: &str = "
Print script parts necessary to activate xan completions, tailored
to your current shell.

Only support `bash` and `zsh` for now.

For `bash`, run:
    $ xan completions bash >> ~/.bashrc

To enable those completions system-wide, you can also run:
    $ xan completions bash > /etc/bash_completions.d/xan

For `zsh`, run:
    $ mkdir -p ~/.zfunc
    $ xan completions zsh > ~/.zfunc/_xan

Then add this before compinit in ~/.zshrc:

    fpath=(~/.zfunc $fpath)
    autoload -Uz compinit
    compinit

You will need to reload your shell or source your main shell
configuration file (`source ~/.bashrc` for bash, for instance) for
the completions to be activated (this is only required once).

Usage:
    xan completions (bash | zsh)
    xan completions --help

Common options:
    -h, --help             Display this message
";

static BASH_COMPLETE_FUNCTION: &str = r#"# Xan completions
function __xan {
    xan compgen "$1" "$2" "$3"
}
complete -C __xan -o default xan
"#;

static ZSH_COMPLETE_FUNCTION: &str = r#"#compdef xan

__xan() {
    local -a completions
    local completion_output

    completion_output=$(COMP_LINE="${(j: :)words}" xan compgen "$words[1]" "$words[CURRENT]" "$words[$((CURRENT - 1))]")

    if [[ -n "$completion_output" ]]; then
        completions=("${(@f)completion_output}")
        compadd -a completions
    else
        _default
    fi
}

if [ "$funcstack[1]" = "_xan" ]; then
    __xan "$@"
else
    compdef __xan xan
fi
"#;

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
