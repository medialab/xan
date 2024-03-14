use csv;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliResult;

static USAGE: &str = "
Implode a CSV file by collapsing multiple consecutive rows into a single one
where the values of some columns are joined using the given separator.

This is the reverse of the 'explode' command.

For instance the following CSV:

name,color
John,blue
John,yellow
Mary,red

Can be imploded on the \"color\" column using the \"|\" <separator> to produce:

name,color
John,blue|yellow
Mary,red

Usage:
    xan implode [options] <columns> <separator> [<input>]
    xan implode --help

implode options:
    -r, --rename <name>    New name for the diverging column.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_columns: SelectColumns,
    arg_separator: String,
    arg_input: Option<String>,
    flag_rename: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

fn compare_but_for_sel(
    first: &csv::ByteRecord,
    second: &csv::ByteRecord,
    except: &[Option<usize>],
) -> bool {
    first
        .iter()
        .zip(second.iter())
        .zip(except.iter())
        .all(|((a, b), mask)| if mask.is_some() { true } else { a == b })
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    // NOTE: the mask deduplicates
    let sel_mask = sel.indexed_mask(headers.len());

    if let Some(new_names) = args.flag_rename {
        let new_names = util::str_to_csv_byte_record(&new_names);

        if new_names.len() != sel.len() {
            return fail!(format!(
                "Renamed columns alignement error. Expected {} names and got {}.",
                sel.len(),
                new_names.len(),
            ));
        }

        headers = headers
            .iter()
            .zip(sel_mask.iter())
            .map(|(h, rh)| if let Some(i) = rh { &new_names[*i] } else { h })
            .collect();
    }

    if !rconfig.no_headers {
        wtr.write_byte_record(&headers)?;
    }

    let sep = args.arg_separator.as_bytes();
    let mut previous: Option<csv::ByteRecord> = None;
    let mut accumulator: Vec<Vec<Vec<u8>>> = Vec::with_capacity(sel.len());

    for result in rdr.into_byte_records() {
        let record = result?;

        if let Some(previous_record) = previous {
            if !compare_but_for_sel(&record, &previous_record, &sel_mask) {
                // Flushing
                let imploded_record: csv::ByteRecord = previous_record
                    .iter()
                    .zip(sel_mask.iter())
                    .map(|(cell, mask)| {
                        if let Some(i) = mask {
                            accumulator
                                .iter()
                                .map(|acc| acc[*i].clone())
                                .collect::<Vec<_>>()
                                .join(sep)
                        } else {
                            cell.to_vec()
                        }
                    })
                    .collect();

                wtr.write_byte_record(&imploded_record)?;

                accumulator.clear();
            }
        }

        accumulator.push(sel.select(&record).map(|c| c.to_vec()).collect());
        previous = Some(record);
    }

    // Flushing last instance
    if !accumulator.is_empty() {
        let imploded_record: csv::ByteRecord = previous
            .unwrap()
            .iter()
            .zip(sel_mask)
            .map(|(cell, mask)| {
                if let Some(i) = mask {
                    accumulator
                        .iter()
                        .map(|acc| acc[i].clone())
                        .collect::<Vec<_>>()
                        .join(sep)
                } else {
                    cell.to_vec()
                }
            })
            .collect();

        wtr.write_byte_record(&imploded_record)?;
    }

    Ok(wtr.flush()?)
}
