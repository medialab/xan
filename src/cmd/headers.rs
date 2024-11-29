use std::collections::BTreeMap;

use colored::Colorize;

use crate::config::Delimiter;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Prints the fields of the first row in the CSV data.

These names can be used in commands like 'select' to refer to columns in the
CSV data.

Usage:
    xan headers [options] [<input>...]
    xan h [options] [<input>...]

headers options:
    -j, --just-names       Only show the header names (hide column index).
                           This is automatically enabled if more than one
                           input is given.

Common options:
    -h, --help             Display this message
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Vec<String>,
    flag_just_names: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let configs = util::many_configs(&args.arg_input, args.flag_delimiter, true, None)?;

    let num_inputs = configs.len();

    let mut headers: Vec<String> = vec![];

    for conf in configs.into_iter() {
        let mut rdr = conf.reader()?;
        for header in rdr.headers()?.iter() {
            headers.push(header.to_string());
        }
    }

    let duplicate_headers: Vec<String> = {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();

        for h in headers.iter() {
            counts.entry(h).and_modify(|c| *c += 1).or_insert(1);
        }

        counts
            .into_iter()
            .filter_map(|(h, c)| if c > 1 { Some(h.to_string()) } else { None })
            .collect()
    };

    let left_column_size = headers.len().saturating_sub(1).to_string().len().max(4);

    for (i, header) in headers.into_iter().enumerate() {
        if num_inputs == 1 && !args.flag_just_names {
            print!(
                "{}",
                util::unicode_aware_rpad(&i.to_string(), left_column_size, " ")
            );
        }

        println!(
            "{}",
            if duplicate_headers.contains(&header) {
                header.red().bold()
            } else {
                header.normal()
            }
        );
    }

    Ok(())
}
