use std::io::{self, Write};

use colored::Colorize;

use crate::moonblade::Program;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Debug command that can be used to evaluate a moonblade expression.

Usage:
    xan eval <expr>
    xan eval --help

Common options:
    -h, --help  Display this message
";

#[derive(Deserialize)]
struct Args {
    arg_expr: String,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let dummy_record = csv::ByteRecord::new();

    let program = Program::parse(&args.arg_expr, &dummy_record)?;

    let value = program.run_with_record(0, &dummy_record)?;

    print!("{} ", "result".cyan());
    io::stdout().write_all(&value.serialize_as_bytes())?;
    println!();
    println!("{}   {}", "type".cyan(), value.type_of());

    Ok(())
}
