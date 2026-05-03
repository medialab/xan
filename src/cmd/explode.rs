use std::fs;
use std::slice;

use bstr::ByteSlice;
use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliError;
use crate::CliResult;

fn singularize(name: &[u8]) -> Vec<u8> {
    let mut vec = name.to_vec();

    if name.ends_with(b"ies") {
        vec.truncate(vec.len() - 3);
        vec.push(b'y');
    } else if name.ends_with(b"oes") {
        vec.truncate(vec.len() - 2);
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

Note that the file can be exploded on multiple well-aligned columns (that
is to say selected cells must all be split into a same number of values).


TODO: amend help here, mention parallelization

Finally, if you need more complex stuff that splitting cells by a separator,
check out the `flatmap` command instead.

Usage:
    xan explode [options] <columns> [<input>]
    xan explode --help

explode options:
    --sep <sep>            Separator to split the cells.
                           [default: |]
    -e, --evaluate <expr>  Evaluate an expression to split cells instead of using
                           a simple separator.
    -f, --evaluate-file <path>
                           Read splitting expression from a file instead.
    -S, --singularize      Singularize (supporting only very simple English-centric cases)
                           the exploded column names. Does not work with -r, --rename.
    -r, --rename <name>    New names for the exploded columns. Must be written
                           in CSV format if exploding multiple columns.
                           See 'xan rename' help for more details.
                           Does not work with -S, --singular.
    -k, --keep             Keep the exploded columns alongside each split.
    -D, --drop-empty       Drop rows when selected cells are empty.
    --pad                  When exploding multiple columns at once, pad shorter splits
                           to align them with the longest one instead of erroring.

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
    arg_columns: SelectedColumns,
    arg_input: Option<String>,
    flag_sep: String,
    flag_singularize: bool,
    flag_rename: Option<String>,
    flag_drop_empty: bool,
    flag_keep: bool,
    flag_pad: bool,
    flag_evaluate: Option<String>,
    flag_evaluate_file: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn resolve(&mut self) -> CliResult<()> {
        if let Some(path) = &self.flag_evaluate_file {
            self.flag_evaluate = Some(fs::read_to_string(path)?);
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve()?;

    if args.flag_singularize && args.flag_rename.is_some() {
        Err("-S/--singular cannot work with -r/--rename!")?;
    }

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns);

    let mut rdr = rconfig.simd_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    if sel.is_empty() {
        return Err(CliError::Other(
            "expecting a non-empty column selection".to_string(),
        ));
    }

    // NOTE: the mask deduplicates
    let sel_mask = sel.indexed_mask(headers.len());

    let new_names_opt = args
        .flag_rename
        .as_ref()
        .map(|n| util::str_to_csv_byte_record(n));

    let mut new_headers = ByteRecord::new();

    for (name, mask) in headers.iter().zip(sel_mask.iter()) {
        if let Some(i) = mask {
            if args.flag_keep {
                new_headers.push_field(name);
            }

            if let Some(new_names) = &new_names_opt {
                new_headers.push_field(&new_names[*i]);
            } else if args.flag_singularize {
                new_headers.push_field(&singularize(name));
            } else {
                new_headers.push_field(name);
            }
        } else {
            new_headers.push_field(name);
        }
    }

    if !rconfig.no_headers {
        wtr.write_byte_record(&new_headers)?;
    }

    let mut record = ByteRecord::new();
    let mut output_record = ByteRecord::new();

    // Fast path with single column explosion, which is the most common
    if sel.len() == 1 {
        let column_index = sel[0];

        while rdr.read_byte_record(&mut record)? {
            let cell = &record[column_index];

            if args.flag_drop_empty && cell.is_empty() {
                continue;
            }

            if args.flag_keep {
                for sub_cell in cell.split_str(&args.flag_sep) {
                    output_record.clear();

                    for (i, cell) in record.iter().enumerate() {
                        if i == column_index {
                            output_record.push_field(cell);
                            output_record.push_field(sub_cell);
                        } else {
                            output_record.push_field(cell);
                        }
                    }

                    wtr.write_byte_record(&output_record)?;
                }
            } else {
                for sub_cell in cell.split_str(&args.flag_sep) {
                    wtr.write_record(record.iter().enumerate().map(|(i, input_cell)| {
                        if i == column_index {
                            sub_cell
                        } else {
                            input_cell
                        }
                    }))?;
                }
            }
        }

        return Ok(wtr.flush()?);
    }

    let mut splits: Vec<Vec<(*const u8, usize)>> = Vec::with_capacity(sel.len());

    for _ in 0..sel.len() {
        splits.push(Vec::new());
    }

    while rdr.read_byte_record(&mut record)? {
        let mut all_empty = true;

        for (i, cell) in sel.select(&record).enumerate() {
            let col_splits = &mut splits[i];
            col_splits.clear();

            if !cell.is_empty() {
                all_empty = false;
            }

            for slice in cell.split_str(&args.flag_sep) {
                col_splits.push((slice.as_ptr(), slice.len()));
            }
        }

        if args.flag_drop_empty && all_empty {
            continue;
        }

        let max_len = if args.flag_pad {
            splits.iter().map(|s| s.len()).max().unwrap()
        } else {
            if splits.iter().skip(1).any(|s| s.len() != splits[0].len()) {
                return Err(CliError::Other(
                    "inconsistent exploded length across columns.".to_string(),
                ));
            }

            splits[0].len()
        };

        for i in 0..max_len {
            if args.flag_keep {
                output_record.clear();

                for (cell, mask) in record.iter().zip(sel_mask.iter()) {
                    output_record.push_field(cell);

                    output_record.push_field(if let Some(j) = mask {
                        if let Some(sub_cell) = splits[*j].get(i) {
                            unsafe { slice::from_raw_parts(sub_cell.0, sub_cell.1) }
                        } else {
                            b"".as_slice()
                        }
                    } else {
                        cell
                    });
                }

                wtr.write_byte_record(&output_record)?;
            } else {
                wtr.write_record(record.iter().zip(sel_mask.iter()).map(|(cell, mask)| {
                    if let Some(j) = mask {
                        if let Some(sub_cell) = splits[*j].get(i) {
                            unsafe { slice::from_raw_parts(sub_cell.0, sub_cell.1) }
                        } else {
                            b"".as_slice()
                        }
                    } else {
                        cell
                    }
                }))?;
            }
        }
    }

    Ok(wtr.flush()?)
}
