use std::fs;

use bstr::ByteSlice;
use simd_csv::ByteRecord;

use crate::CliError;
use crate::CliResult;
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::utils;

use crate::moonblade::{AmortizedSerializer, Program};

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

enum Exploder {
    Substring(String),
    Program {
        program: Program,
        serializer: AmortizedSerializer,
    },
}

impl Exploder {
    fn explode<F>(
        &mut self,
        drop_empty: bool,
        row_index: usize,
        column_index: usize,
        record: &ByteRecord,
        cell: &[u8],
        mut callback: F,
    ) -> CliResult<bool>
    where
        F: FnMut(&[u8]) -> CliResult<()>,
    {
        match self {
            Self::Substring(sep) => {
                if drop_empty && cell.is_empty() {
                    return Ok(true);
                }

                // TODO: optimized routine for single char, and reuse Finder
                for sub in cell.split_str(sep) {
                    callback(sub)?;
                }
            }
            Self::Program {
                program,
                serializer,
            } => {
                let value =
                    program.run_with_record_and_col_index(row_index, column_index, &record)?;

                if drop_empty && value.is_nullish() {
                    return Ok(true);
                }

                // TODO: flatmap mode
                match value.try_unwrap_as_vec() {
                    Err(value) => Err(format!(
                        "when using -e/--evaluate, produced value should be a list but got {}!",
                        value.type_of()
                    ))?,
                    Ok(list) => {
                        if list.is_empty() {
                            callback(cell)?;
                        } else {
                            for item in list {
                                callback(serializer.serialize(&item))?;
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }
}

static USAGE: &str = "
Explode CSV rows into multiple ones by splitting selected cell using the pipe
character (\"|\") or any separator given to the --sep flag.

This command is conceptually the inverse of the \"implode\" command.

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

Note that a file can be exploded on multiple well-aligned columns that would
be split into a same number of values. Else you can always use the --pad flag.

Alternatively, you can also use an expression using the -e <expr> flag if you
need to split your cells using custom logic, e.g. parsing JSON etc.

    $ xan explode json_names -e '_.parse_json().compact()' file.csv

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
    -D, --drop-empty       Drop rows when selected cells are empty, or when the evaluated
                           expression yields a nullish value.
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

    #[inline]
    fn max_len(&self, splits: &[ByteRecord]) -> CliResult<usize> {
        if self.flag_pad {
            Ok(splits.iter().map(ByteRecord::len).max().unwrap())
        } else {
            if splits.iter().skip(1).any(|s| s.len() != splits[0].len()) {
                Err("inconsistent exploded length across columns.")?;
            }

            Ok(splits[0].len())
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = utils::get_args(USAGE, argv)?;
    args.resolve()?;

    if args.flag_singularize && args.flag_rename.is_some() {
        Err("-S/--singular cannot work with -r/--rename!")?;
    }

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns.clone());

    let mut rdr = rconfig.simd_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;

    if sel.is_empty() {
        return Err(CliError::Other(
            "expecting a non-empty column selection".to_string(),
        ));
    }

    let mut exploder = if let Some(expr) = &args.flag_evaluate {
        Exploder::Program {
            program: Program::parse(expr, &headers, rconfig.no_headers)?,
            serializer: AmortizedSerializer::new(),
        }
    } else {
        Exploder::Substring(args.flag_sep.clone())
    };

    // NOTE: the mask deduplicates
    let sel_mask = sel.indexed_mask(headers.len());

    let new_names_opt = args
        .flag_rename
        .as_ref()
        .map(|n| utils::str_to_csv_byte_record(n));

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
    let mut row_index: usize = 0;

    // Fast path with single column explosion, which is the most common
    if sel.len() == 1 {
        let column_index = sel[0];

        while rdr.read_byte_record(&mut record)? {
            let cell = &record[column_index];
            row_index += 1;

            exploder.explode(
                args.flag_drop_empty,
                row_index - 1,
                column_index,
                &record,
                cell,
                |sub_cell| {
                    wtr.write_fields(|w| {
                        for (i, cell) in record.iter().enumerate() {
                            if i == column_index {
                                if args.flag_keep {
                                    w.write_field(cell)?;
                                }

                                w.write_field(sub_cell)?;
                            } else {
                                w.write_field(cell)?;
                            }
                        }

                        Ok(())
                    })?;

                    Ok(())
                },
            )?;
        }

        return Ok(wtr.flush()?);
    }

    // Slow path with multiple exploded columns
    let mut splits: Vec<ByteRecord> = Vec::with_capacity(sel.len());

    for _ in 0..sel.len() {
        splits.push(ByteRecord::new());
    }

    while rdr.read_byte_record(&mut record)? {
        let mut all_empty = true;
        row_index += 1;

        for (i, column_index) in sel.iter().copied().enumerate() {
            let col_splits = &mut splits[i];
            let cell = &record[column_index];
            col_splits.clear();

            all_empty &= exploder.explode(
                args.flag_drop_empty,
                row_index,
                column_index,
                &record,
                cell,
                |sub_cell| {
                    col_splits.push_field(sub_cell);
                    Ok(())
                },
            )?;
        }

        if args.flag_drop_empty && all_empty {
            continue;
        }

        let max_len = args.max_len(&splits)?;

        for i in 0..max_len {
            wtr.write_fields(|w| {
                for (cell, mask) in record.iter().zip(sel_mask.iter()) {
                    if let Some(j) = mask {
                        if args.flag_keep {
                            w.write_field(cell)?;
                        }

                        if let Some(sub_cell) = splits[*j].get(i) {
                            w.write_field(sub_cell)?;
                        } else {
                            w.write_field(b"")?;
                        }
                    } else {
                        w.write_field(cell)?;
                    }
                }

                Ok(())
            })?;
        }
    }

    Ok(wtr.flush()?)
}
