use colored::Colorize;
use lazy_static::lazy_static;
use regex::{Captures, Regex};
use textwrap::{indent, wrap};

use crate::util;
use crate::CliResult;

fn get_operators_summary_txt() -> &'static str {
    "- Operators
    - Unary operators
    - Numerical comparison
    - String/sequence comparison
    - Arithmetic operators
    - String/sequence operators
    - Logical operators
    - Indexing & slicing operators
    - Pipeline operator
"
}

fn get_cheatsheet_str() -> &'static str {
    include_str!("../moonblade/doc/cheatsheet.txt")
}

fn get_functions_operators_help_str() -> &'static str {
    include_str!("../moonblade/doc/operators.txt")
}

fn get_functions_help_prelude_str() -> &'static str {
    include_str!("../moonblade/doc/functions_prelude.txt")
}

fn get_functions_help_json_str() -> &'static str {
    include_str!("../moonblade/doc/functions.json")
}

fn get_aggs_help_prelude_str() -> &'static str {
    include_str!("../moonblade/doc/aggs_prelude.txt")
}

fn get_aggs_help_json_str() -> &'static str {
    include_str!("../moonblade/doc/aggs.json")
}

fn escape_markdown_star(string: &str) -> String {
    string.replace("*", "\\*")
}

fn escape_markdown_linebreak(string: &str) -> String {
    string.replace("\n", "<br>")
}

fn slug(string: &str) -> String {
    string
        .to_lowercase()
        .replace(|c: char| c != ' ' && !c.is_ascii_alphanumeric(), "")
        .replace(' ', "-")
}

#[derive(Deserialize, Debug)]
struct FunctionHelpSections(Vec<FunctionHelpSection>);

impl FunctionHelpSections {
    fn to_txt(&self, section: &Option<String>) -> String {
        let mut should_keep_operators = true;

        let filtered_sections: Vec<&FunctionHelpSection> = if let Some(query) = section {
            if !"operators".contains(&query.to_lowercase()) {
                should_keep_operators = false;
            }

            self.0
                .iter()
                .filter(|s| s.section.to_lowercase().contains(&query.to_lowercase()))
                .collect()
        } else {
            self.0.iter().collect()
        };

        let mut string = String::new();

        // Prelude
        string.push_str(&colorize_functions_help(get_functions_help_prelude_str()));
        string.push('\n');

        // Summary
        if should_keep_operators {
            string.push_str(get_operators_summary_txt());
        }
        string.push_str(
            &filtered_sections
                .iter()
                .map(|section| format!("- {}", section.section))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        string.push_str("\n\n");

        // Operators
        if should_keep_operators {
            string.push_str(&colorize_functions_help(get_functions_operators_help_str()));
            string.push('\n');
        }

        // Sections
        string.push_str(
            &filtered_sections
                .iter()
                .map(|section| section.to_txt())
                .collect::<Vec<_>>()
                .join(""),
        );

        string
    }

    fn to_md(&self) -> String {
        let mut string = String::new();

        // Prelude
        string.push_str(get_functions_help_prelude_str());
        string.push('\n');

        // Summary
        // TODO: proper links & slugs
        string.push_str(get_operators_summary_txt());
        string.push_str(
            &self
                .0
                .iter()
                .map(|section| format!("- [{}](#{})", section.section, slug(&section.section)))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        string.push_str("\n\n");

        // Operators
        string.push_str(get_functions_operators_help_str());
        string.push('\n');

        // Sections
        string.push_str(
            &self
                .0
                .iter()
                .map(|section| section.to_md())
                .collect::<Vec<_>>()
                .join(""),
        );

        string
    }
}

#[derive(Deserialize, Debug)]
struct FunctionHelpSection {
    section: String,
    functions: Vec<FunctionHelp>,
}

impl FunctionHelpSection {
    fn to_txt(&self) -> String {
        let mut string = String::new();

        string.push_str(&format!("## {}\n\n", self.section).yellow().to_string());

        for function in self.functions.iter() {
            string.push_str(&indent(&function.to_txt(), "    "));
        }

        string.push('\n');
        string
    }

    fn to_md(&self) -> String {
        let mut string = String::new();

        string.push_str(&format!("## {}\n\n", self.section));

        for function in self.functions.iter() {
            string.push_str(&function.to_md());
        }

        string.push('\n');
        string
    }
}

#[derive(Deserialize, Debug)]
struct FunctionHelp {
    name: String,
    arguments: Vec<String>,
    returns: String,
    help: String,
    aliases: Option<Vec<String>>,
    alternatives: Option<Vec<Vec<String>>>,
}

fn join_arguments(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg.starts_with('<') {
                arg.dimmed().to_string()
            } else {
                arg.red().to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

impl FunctionHelp {
    fn to_txt(&self) -> String {
        let mut string = String::new();

        // Main call
        string.push_str(&format!("- {}(", self.name.cyan()));
        string.push_str(&format!(
            "{}) -> {}\n",
            join_arguments(&self.arguments),
            self.returns.magenta()
        ));

        // Aliases
        for alias in self.aliases.iter().flatten() {
            string.push_str(&format!("- {}(", alias.cyan()));
            string.push_str(&format!(
                "{}) -> {}\n",
                join_arguments(&self.arguments),
                self.returns.magenta()
            ));
        }

        // Alternatives
        for alternative in self.alternatives.iter().flatten() {
            string.push_str(&format!("- {}(", self.name.cyan()));
            string.push_str(&format!(
                "{}) -> {}\n",
                join_arguments(alternative),
                self.returns.magenta()
            ));
        }

        string.push_str(&colorize_functions_help(&indent(
            &wrap(&self.help, 80).join("\n"),
            "    ",
        )));
        string.push_str("\n\n");

        string
    }

    fn to_md(&self) -> String {
        fn single_form(name: &str, args: &[String], returns: &str, help: &str) -> String {
            format!(
                "- **{}**({}) -> `{}`: {}",
                name,
                args.iter()
                    .map(|arg| { format!("*{}*", escape_markdown_star(arg)) })
                    .collect::<Vec<_>>()
                    .join(", "),
                returns,
                escape_markdown_linebreak(help)
            )
        }

        let mut string = String::new();

        // Main call
        string.push_str(&single_form(
            &self.name,
            &self.arguments,
            &self.returns,
            &self.help,
        ));

        // Aliases
        for alias in self.aliases.iter().flatten() {
            string.push_str(&single_form(
                alias,
                &self.arguments,
                &self.returns,
                &self.help,
            ));
        }

        // Alternatives
        for alternative in self.alternatives.iter().flatten() {
            string.push_str(&single_form(
                &self.name,
                alternative,
                &self.returns,
                &self.help,
            ));
        }

        string.push('\n');

        string
    }
}

#[derive(Debug, Deserialize)]
struct Aggs(Vec<FunctionHelp>);

impl Aggs {
    fn to_txt(&self) -> String {
        let mut string = String::new();

        string.push_str(&colorize_functions_help(get_aggs_help_prelude_str()));
        string.push('\n');

        string.push_str(
            &self
                .0
                .iter()
                .map(|help| indent(&help.to_txt(), "    "))
                .collect::<Vec<_>>()
                .join(""),
        );

        string
    }
}

lazy_static! {
    static ref MAIN_SECTION_REGEX: Regex = Regex::new("(?m)^##{0,2} .+").unwrap();
    static ref FLAG_REGEX: Regex = Regex::new(r"--[\w\-]+").unwrap();
    static ref UNARY_OPERATOR_REGEX: Regex = Regex::new(r"([!-])x").unwrap();
    static ref BINARY_OPERATOR_REGEX: Regex = Regex::new(
        r"x (==|!=|<[= ]|>[= ]|&& |\|\| |and|or |not in|in|eq|ne|lt|le|gt|ge|//|\*\*|\+\+|[+\-*/%]) y"
    )
    .unwrap();
    static ref PIPELINE_OPERATOR_REGEX: Regex = Regex::new(
        r"(trim\(name\) )\|"
    )
    .unwrap();
    static ref SLICE_REGEX: Regex = Regex::new(r"x\[([a-z:]+)\]").unwrap();
    static ref QUOTE_REGEX: Regex = Regex::new(r#"(?m)"[^"\n]+"|'[^'\n]+'|`[^`\n]+`"#).unwrap();
    static ref CHEATSHEET_ITEM_REGEX: Regex = Regex::new(r"(?m)^  \. (.+)$").unwrap();
}

fn colorize_cheatsheet(help: &str) -> String {
    let help = CHEATSHEET_ITEM_REGEX.replace_all(help, |caps: &Captures| {
        "  . ".to_string() + &caps[1].yellow().to_string()
    });

    let help = FLAG_REGEX.replace_all(&help, |caps: &Captures| caps[0].cyan().to_string());

    help.into_owned()
}

fn colorize_functions_help(help: &str) -> String {
    let help = QUOTE_REGEX.replace_all(help, |caps: &Captures| caps[0].green().to_string());

    let help =
        MAIN_SECTION_REGEX.replace_all(&help, |caps: &Captures| caps[0].yellow().to_string());

    let help = UNARY_OPERATOR_REGEX.replace_all(&help, |caps: &Captures| {
        caps[1].cyan().to_string() + &"x".red().to_string()
    });

    let help = BINARY_OPERATOR_REGEX.replace_all(&help, |caps: &Captures| {
        "x".red().to_string() + " " + &caps[1].cyan().to_string() + " " + &"y".red().to_string()
    });

    let help = PIPELINE_OPERATOR_REGEX.replace_all(&help, |caps: &Captures| {
        caps[1].to_string() + &"|".cyan().to_string()
    });

    let help = SLICE_REGEX.replace_all(&help, |caps: &Captures| {
        "x".red().to_string()
            + "["
            + &caps[1]
                .split(':')
                .map(|part| part.cyan().to_string())
                .collect::<Vec<_>>()
                .join(":")
            + "]"
    });

    let help = FLAG_REGEX.replace_all(&help, |caps: &Captures| caps[0].cyan().to_string());

    help.into_owned()
}

fn get_colorized_cheatsheet() -> String {
    colorize_cheatsheet(get_cheatsheet_str())
}

fn parse_functions_help() -> FunctionHelpSections {
    let json_str = get_functions_help_json_str();
    serde_json::from_str(json_str).unwrap()
}

fn parse_aggs_help() -> Aggs {
    let json_str = get_aggs_help_json_str();
    Aggs(serde_json::from_str::<Vec<FunctionHelp>>(json_str).unwrap())
}

static USAGE: &str = "
Print help about the `xan` expression language.

`xan help cheatsheet` will print a short cheatsheet about
how the language works.

`xan help functions` will print the reference of all of the language's
functions (used in `xan select -e`, `xan map`, `xan filter`, `xan transform`,
`xan flatmap` etc.).

`xan help aggs` will print the reference of all of the language's
aggregation functions (as used in `xan agg` and `xan groupby` mostly).

Usage:
    xan help cheatsheet [options]
    xan help functions [options]
    xan help aggs [options]
    xan help --help

help options:
    -p, --pager            Pipe the help into a pager (Same as piping
                           with forced colors into `less -SRi`).
    -S, --section <query>  Filter the `functions` doc to only include
                           sections matching the given case-insensitive
                           query.
    --json                 Dump the help as JSON data.
    --md                   Dump the help as Markdown.

Common options:
    -h, --help             Display this message
";

#[derive(Deserialize)]
struct Args {
    cmd_cheatsheet: bool,
    cmd_functions: bool,
    cmd_aggs: bool,
    flag_pager: bool,
    flag_section: Option<String>,
    flag_json: bool,
    flag_md: bool,
}

impl Args {
    fn setup_pager(&self) {
        if !self.flag_pager {
            return;
        }

        #[cfg(not(windows))]
        {
            colored::control::set_override(true);
            pager::Pager::with_pager("less -SRi").setup();
        }

        #[cfg(windows)]
        {
            Err("The -p/--pager flag does not work on windows, sorry :'(".to_string())?;
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_pager && (args.flag_json || args.flag_md) {
        Err("-p/--pager does not work with --json nor --md!")?;
    }

    if args.flag_section.is_some() && !args.cmd_functions {
        Err("-S/--section <query> only works with the `functions` subcommand!")?;
    }

    if args.cmd_cheatsheet {
        if args.flag_json {
            Err("cheatsheet does not support --json!")?;
        }

        if args.flag_md {
            println!("{}", get_cheatsheet_str());
        } else {
            args.setup_pager();
            println!("{}", get_colorized_cheatsheet());
        }
    } else if args.cmd_functions {
        if args.flag_json {
            println!("{}", get_functions_help_json_str());
        } else if args.flag_md {
            print!("{}", parse_functions_help().to_md());
        } else {
            args.setup_pager();
            print!("{}", parse_functions_help().to_txt(&args.flag_section));
        }
    } else if args.cmd_aggs {
        if args.flag_json {
            println!("{}", get_aggs_help_json_str());
        } else if args.flag_md {
            unimplemented!()
        } else {
            args.setup_pager();
            print!("{}", parse_aggs_help().to_txt());
        }
    }

    Ok(())
}
