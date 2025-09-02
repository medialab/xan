use std::io::{self, stdout, Write};

use colored::Colorize;

use crate::moonblade::Program;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Debug command that can be used to evaluate a moonblade expression.

Usage:
    xan eval [options] <expr>
    xan eval --help

eval options:
    -S, --serialize        Serialize the value in CSV.
    -E, --explain          Print concrete expression plan.
    -H, --headers <names>  Pretend headers, separated by commas, to consider.
    -R, --row <values>     Pretend row with comma-separated cells.

Common options:
    -h, --help  Display this message
";

#[derive(Deserialize)]
struct Args {
    arg_expr: String,
    flag_serialize: bool,
    flag_explain: bool,
    flag_headers: Option<String>,
    flag_row: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let mut out = stdout();

    let mut dummy_headers = csv::ByteRecord::new();

    if let Some(headers) = &args.flag_headers {
        for h in headers.split(',') {
            dummy_headers.push_field(h.as_bytes());
        }
    }

    let program = Program::parse(&args.arg_expr, &dummy_headers)?;

    if args.flag_explain {
        writeln!(&mut out, "{}", "concrete plan".cyan())?;
        writeln!(&mut out, "{:#?}\n", program.expr)?;
    }

    let mut dummy_row = csv::ByteRecord::new();

    if let Some(cells) = args.flag_row {
        for c in cells.split(',') {
            dummy_row.push_field(c.as_bytes());
        }
    } else if let Some(headers) = args.flag_headers {
        for _ in headers.split(',') {
            dummy_row.push_field(b"");
        }
    }

    let value = program.run_with_record(0, &dummy_row)?;

    if args.flag_serialize {
        print!("{} ", "result".cyan());
        io::stdout().write_all(&value.serialize_as_bytes())?;
        writeln!(&mut out)?;
        writeln!(&mut out, "{}   {}", "type".cyan(), value.type_of())?;
    } else {
        writeln!(&mut out, "{} ", "result".cyan())?;
        writeln!(&mut out, "{:#?}", value)?;
    }

    Ok(())
}
