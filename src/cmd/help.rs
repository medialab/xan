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

#[derive(Deserialize, Debug)]
struct FunctionHelpSections(Vec<FunctionHelpSection>);

impl FunctionHelpSections {
    fn sections_summary_txt(&self) -> String {
        self.0
            .iter()
            .map(|section| format!("- {}", section.section))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn to_txt(&self) -> String {
        let mut string = String::new();

        string.push_str(&colorize_functions_help(get_functions_help_prelude_str()));
        string.push('\n');

        string.push_str(get_operators_summary_txt());
        string.push_str(&self.sections_summary_txt());
        string.push_str("\n\n");

        string.push_str(&colorize_functions_help(get_functions_operators_help_str()));
        string.push('\n');

        string.push_str(
            &self
                .0
                .iter()
                .map(|section| section.to_txt())
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
    static ref FUNCTION_REGEX: Regex =
        Regex::new(r"(?i)- ([a-z0-9_]+)\(((?:[a-z0-9=?*_<>]+\s*,?\s*)*)\) -> ([a-z\[\],?| ]+)").unwrap();
    // static ref SPACER_REGEX: Regex = Regex::new(r"(?m)^ {8}([^\n]+)").unwrap();
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
    let help = FUNCTION_REGEX.replace_all(help, |caps: &Captures| {
        "- ".to_string()
            + &caps[1].cyan().to_string()
            + &"(".yellow().to_string()
            + &caps[2]
                .split(", ")
                .map(|arg| {
                    (if arg == "<expr>" || arg == "<expr>?" {
                        arg.dimmed()
                    } else {
                        arg.red()
                    })
                    .to_string()
                })
                .collect::<Vec<_>>()
                .join(", ")
            + &")".yellow().to_string()
            + " -> "
            + &caps[3].magenta().to_string()
    });

    let help = QUOTE_REGEX.replace_all(&help, |caps: &Captures| caps[0].green().to_string());

    let help =
        MAIN_SECTION_REGEX.replace_all(&help, |caps: &Captures| caps[0].yellow().to_string());

    // let help = SPACER_REGEX.replace_all(&help, |caps: &Captures| {
    //     " ".repeat(8) + &caps[1].dimmed().to_string()
    // });

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
    --json  Dump the desired help as JSON.

Common options:
    -h, --help             Display this message
";

#[derive(Deserialize)]
struct Args {
    cmd_cheatsheet: bool,
    cmd_functions: bool,
    cmd_aggs: bool,
    flag_json: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.cmd_cheatsheet {
        if args.flag_json {
            Err("cheatsheet does not support --json!")?;
        }

        println!("{}", get_colorized_cheatsheet());
    } else if args.cmd_functions {
        if args.flag_json {
            println!("{}", get_functions_help_json_str());
        } else {
            print!("{}", parse_functions_help().to_txt());
        }
    } else if args.cmd_aggs {
        if args.flag_json {
            println!("{}", get_aggs_help_json_str());
        } else {
            print!("{}", parse_aggs_help().to_txt());
        }
    }

    Ok(())
}
