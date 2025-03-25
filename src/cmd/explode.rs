use bstr::ByteSlice;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliError;
use crate::CliResult;

fn singularize(name: &[u8]) -> Vec<u8> {
    let mut vec = name.to_vec();

    if name.ends_with(b"ies") {
        vec.truncate(vec.len() - 3);
        vec.push(b'y');
    } else if name.ends_with(b"s") {
        vec.truncate(vec.len() - 1);
    }

    vec
}

static USAGE: &str = "
Explode CSV rows into multiple ones by splitting selected cell using the pipe
character (\"|\") or any separator given to the --sep flag.

This is conceptually the inverse of the \"implode\" command.

For instance the following CSV:

*file.csv*
name,colors
John,blue|yellow
Mary,red

Can be exploded on the \"colors\" column:

    $ xan explode colors --singular file.csv > exploded.csv

To produce the following file:

*exploded.csv*
name,color
John,blue
John,yellow
Mary,red

Note finally that the file can be exploded on multiple well-aligned columns (that
is to say selected cells must all be splitted into a same number of values).

Usage:
    xan explode [options] <columns> [<input>]
    xan explode --help

explode options:
    --sep <sep>          Separator to split the cells.
                         [default: |]
    -S, --singularize    Singularize (supporting only very simple English-centric cases)
                         the exploded column names. Does not work with -r, --rename.
    -r, --rename <name>  New names for the exploded columns. Must be written
                         in CSV format if exploding multiple columns.
                         See 'xan rename' help for more details.
                         Does not work with -S, --singular.
    -D, --drop-empty     Drop rows when selected cells are empty.

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
    flag_singularize: bool,
    flag_rename: Option<String>,
    flag_drop_empty: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_singularize && args.flag_rename.is_some() {
        Err("-S/--singular cannot work with -r/--rename!")?;
    }

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    if sel.is_empty() {
        return Err(CliError::Other(
            "expecting a non-empty column selection".to_string(),
        ));
    }

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

    if args.flag_singularize {
        headers = headers
            .iter()
            .zip(sel_mask.iter())
            .map(|(h, m)| {
                if m.is_some() {
                    singularize(h)
                } else {
                    h.to_vec()
                }
            })
            .collect();
    }

    if !rconfig.no_headers {
        wtr.write_byte_record(&headers)?;
    }

    let mut record = csv::ByteRecord::new();

    'main: while rdr.read_byte_record(&mut record)? {
        let mut splits: Vec<Vec<&[u8]>> = Vec::with_capacity(sel.len());

        for cell in sel.select(&record) {
            if args.flag_drop_empty && cell.is_empty() {
                continue 'main;
            }

            splits.push(cell.split_str(&args.flag_sep).collect());
        }

        if splits.iter().skip(1).any(|s| s.len() != splits[0].len()) {
            return Err(CliError::Other(
                "inconsistent exploded length accross columns.".to_string(),
            ));
        }

        for i in 0..splits[0].len() {
            let output_record: csv::ByteRecord = record
                .iter()
                .zip(sel_mask.iter())
                .map(|(cell, mask)| {
                    if let Some(j) = mask {
                        splits[*j][i]
                    } else {
                        cell
                    }
                })
                .collect();

            wtr.write_byte_record(&output_record)?;
        }
    }

    Ok(wtr.flush()?)
}
