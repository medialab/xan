use std::collections::BTreeMap;

use colored::Colorize;

use crate::config::{Config, Delimiter};
use crate::util::{self, ColorMode};
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
    -s, --start <n>   Column indices will start from given number.
                      [default: 0]
    --color <when>    When to color the output using ANSI escape codes.
                      Use `auto` for automatic detection, `never` to
                      disable colors completely and `always` to force
                      colors, even when the output could not handle them.
                      [default: auto]

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
    flag_start: usize,
    flag_color: ColorMode,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    args.flag_color.apply();

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

    let mut out = Config::new(&args.flag_output).io_writer()?;

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
            writeln!(
                &mut out,
                "{}",
                conf.path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| "<stdin>".to_string())
                    .cyan()
            )?;
        }

        let duplicates = find_duplicates(&headers);

        for (j, header) in headers.into_iter().enumerate() {
            if !args.flag_just_names {
                write!(
                    &mut out,
                    "{}",
                    util::unicode_aware_rpad(
                        &(args.flag_start + j).to_string(),
                        left_column_size,
                        " "
                    )
                )?;
            }

            let display_header = util::highlight_problematic_string_features(
                &util::sanitize_text_for_single_line_printing(&header),
            );

            writeln!(
                &mut out,
                "{}",
                if duplicates.contains(&header) {
                    display_header.red()
                } else if *name_counts.get(&header).unwrap() < configs.len() {
                    display_header.dimmed()
                } else {
                    display_header.normal()
                }
            )?;
        }

        if i < configs.len() - 1 {
            writeln!(&mut out)?;
        }
    }

    if !single_input {
        let same_headers_everywhere = name_counts.values().all(|c| *c == configs.len());

        if same_headers_everywhere {
            writeln!(&mut out, "{}", "\nAll files have the same headers!".green())?;
        } else {
            writeln!(
                &mut out,
                "{}",
                "\nAll files don't have the same headers!".yellow()
            )?;

            let diverging_headers = name_counts
                .iter()
                .filter_map(|(name, count)| {
                    if *count < configs.len() {
                        Some(
                            util::highlight_problematic_string_features(
                                &util::sanitize_text_for_single_line_printing(name),
                            )
                            .cyan()
                            .to_string(),
                        )
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            writeln!(
                &mut out,
                "{} {}",
                "Diverging headers:".yellow(),
                diverging_headers
            )?;
        }
    }

    Ok(())
}
