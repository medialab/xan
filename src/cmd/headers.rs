use std::collections::BTreeMap;

use colored::Colorize;

use crate::config::{Config, Delimiter};
use crate::util;
use crate::CliResult;

fn find_duplicates(headers: &[String]) -> Vec<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();

    for h in headers.iter() {
        counts.entry(h).and_modify(|c| *c += 1).or_insert(1);
    }

    counts
        .into_iter()
        .filter_map(|(h, c)| if c > 1 { Some(h.to_string()) } else { None })
        .collect()
}

static USAGE: &str = "
Print the headers of CSV files, with duplicated column names printed in yellow.

When given multiple files, headers found across all files will be kept in white,
while diverging headers will be printed in grey.

Usage:
    xan headers [options] [<input>...]
    xan h [options] [<input>...]

headers options:
    -j, --just-names  Only show the header names (hide column index).
    --csv             Return headers as a CSV file, with file path as
                      column names.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Vec<String>,
    flag_just_names: bool,
    flag_csv: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let configs = util::many_configs(&args.arg_input, args.flag_delimiter, true, None)?;

    let mut headers_per_input: Vec<Vec<String>> = Vec::with_capacity(configs.len());

    let single_input = configs.len() == 1;

    for conf in configs.iter() {
        headers_per_input.push(
            conf.reader()?
                .headers()?
                .iter()
                .map(|h| h.to_string())
                .collect(),
        );
    }

    if args.flag_csv {
        let mut wtr = Config::new(&args.flag_output).writer()?;

        let mut record = csv::StringRecord::new();

        for conf in configs.iter() {
            record.push_field(
                &conf
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "<stdin>".to_string()),
            );
        }

        wtr.write_record(&record)?;

        let max_len = headers_per_input.iter().map(|h| h.len()).max().unwrap();

        for i in 0..max_len {
            record.clear();

            for headers in headers_per_input.iter() {
                record.push_field(headers.get(i).unwrap_or(&"".to_string()));
            }

            wtr.write_record(&record)?;
        }

        return Ok(wtr.flush()?);
    }

    let mut name_counts = BTreeMap::<String, usize>::new();

    for headers in headers_per_input.iter() {
        for name in headers.iter() {
            name_counts
                .entry(name.to_string())
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }

    let left_column_size = headers_per_input
        .iter()
        .map(|h| h.len().saturating_sub(1).to_string().len().max(4))
        .max()
        .unwrap();

    for (i, (headers, conf)) in headers_per_input
        .into_iter()
        .zip(configs.iter())
        .enumerate()
    {
        if !single_input {
            println!(
                "{}",
                conf.path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "<stdin>".to_string())
                    .cyan()
            );
        }

        let duplicates = find_duplicates(&headers);

        for (j, header) in headers.into_iter().enumerate() {
            if !args.flag_just_names {
                print!(
                    "{}",
                    util::unicode_aware_rpad(&j.to_string(), left_column_size, " ")
                );
            }

            println!(
                "{}",
                if duplicates.contains(&header) {
                    header.red()
                } else if *name_counts.get(&header).unwrap() < configs.len() {
                    header.dimmed()
                } else {
                    header.normal()
                }
            );
        }

        if i < configs.len() - 1 {
            println!();
        }
    }

    if !single_input && name_counts.values().all(|c| *c == configs.len()) {
        println!("{}", "\nAll files have the same headers!".green());
    }

    Ok(())
}
