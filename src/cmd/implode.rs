use crate::config::{Config, Delimiter};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

fn pluralize(name: &[u8]) -> Vec<u8> {
    let mut vec = name.to_vec();

    if name.ends_with(b"y") {
        vec.truncate(vec.len() - 1);
        vec.extend(b"ies");
    } else {
        vec.push(b's');
    }

    vec
}

static USAGE: &str = "
Implode a CSV file by merging multiple consecutive rows into a single one, where
diverging cells will be joined by the pipe character (\"|\") or any separator
given to the --sep flag.

This is the inverse of the \"explode\" command.

For instance the following CSV:

*file.csv*
name,color
John,blue
John,yellow
Mary,red

Can be imploded on the \"color\" column:

    $ xan implode color --plural file.csv > imploded.csv

To produce the following file:

*imploded.csv*
name,color
John,blue|yellow
Mary,red

Usage:
    xan implode [options] <columns> [<input>]
    xan implode --help

implode options:
    --sep <sep>          Separator that will be used to join the diverging cells.
                         [default: |]
    -P, --plural         Pluralize (supporting only very simple English-centric cases)
                         the imploded column names. Does not work with -r, --rename.
    -r, --rename <name>  New name for the diverging column.
                         Does not work with -P, --plural.
    --cmp <column>       Restrict the columns to compare to assert whether
                         consecutive rows must be merged. Be aware that this will
                         ignore all other columns to in the given selection so
                         only use this as an optimization trick (because you have some
                         column containing a unique id and/or can guarantee all other
                         cells will be identical).

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
    arg_input: Option<String>,
    flag_sep: String,
    flag_plural: bool,
    flag_rename: Option<String>,
    flag_cmp: Option<SelectColumns>,
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

fn compare_sel(first: &csv::ByteRecord, second: &csv::ByteRecord, sel: &Selection) -> bool {
    sel.select(first)
        .zip(sel.select(second))
        .all(|(a, b)| a == b)
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_plural && args.flag_rename.is_some() {
        Err("-P/--plural cannot work with -r/--rename!")?;
    }

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;
    let cmp_sel_opt = args
        .flag_cmp
        .map(|s| s.selection(&headers, !args.flag_no_headers))
        .transpose()?;

    // NOTE: the mask deduplicates
    let sel_mask = sel.indexed_mask(headers.len());

    if let Some(new_names) = args.flag_rename {
        let new_names = util::str_to_csv_byte_record(&new_names);

        if new_names.len() != sel.len() {
            Err(format!(
                "Renamed columns alignement error. Expected {} names and got {}.",
                sel.len(),
                new_names.len(),
            ))?;
        }

        headers = headers
            .iter()
            .zip(sel_mask.iter())
            .map(|(h, rh)| if let Some(i) = rh { &new_names[*i] } else { h })
            .collect();
    }

    if args.flag_plural {
        headers = headers
            .iter()
            .zip(sel_mask.iter())
            .map(|(h, m)| {
                if m.is_some() {
                    pluralize(h)
                } else {
                    h.to_vec()
                }
            })
            .collect()
    }

    if !rconfig.no_headers {
        wtr.write_byte_record(&headers)?;
    }

    let sep = args.flag_sep.as_bytes();
    let mut previous: Option<csv::ByteRecord> = None;
    let mut accumulator: Vec<Vec<Vec<u8>>> = Vec::with_capacity(sel.len());

    for result in rdr.into_byte_records() {
        let record = result?;

        if let Some(previous_record) = previous {
            let should_flush = if let Some(cmp_sel) = &cmp_sel_opt {
                !compare_sel(&record, &previous_record, cmp_sel)
            } else {
                !compare_but_for_sel(&record, &previous_record, &sel_mask)
            };

            if should_flush {
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
