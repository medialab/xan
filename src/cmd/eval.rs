use std::io::{self, Write};

use colored::Colorize;

use crate::CliResult;
use crate::moonblade::Program;
use crate::util;

static USAGE: &str = "
Evaluate a single moonblade expression.

Usage:
    xan eval [options] <expr>
    xan eval --help

eval options:
    -E, --explain          Print concrete expression plan and pretty print
                           evaluated result.
    -H, --headers <names>  Pretend headers, separated by commas.
    -R, --row <values>     Pretend row with comma-separated cells.

Common options:
    -h, --help  Display this message
";

#[derive(Deserialize)]
struct Args {
    arg_expr: String,
    flag_explain: bool,
    flag_headers: Option<String>,
    flag_row: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let mut out = io::stdout();

    let mut dummy_headers = simd_csv::ByteRecord::new();

    if let Some(headers) = &args.flag_headers {
        for h in headers.split(',') {
            dummy_headers.push_field(h.as_bytes());
        }
    } else if let Some(cells) = &args.flag_row {
        for _ in cells.split(',') {
            dummy_headers.push_field(b"");
        }
    }

    let program = Program::parse(&args.arg_expr, &dummy_headers, false)?;

    if args.flag_explain {
        writeln!(&mut out, "{}", "concrete plan".cyan())?;
        writeln!(&mut out, "{:#?}\n", program.expr)?;
    }

    let mut dummy_row = simd_csv::ByteRecord::new();

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

    if !args.flag_explain {
        out.write_all(&value.serialize_as_bytes())?;
        writeln!(&mut out)?;
    } else {
        writeln!(&mut out, "{} ", "result".cyan())?;
        writeln!(&mut out, "{:#?}", value)?;
    }

    Ok(())
}
