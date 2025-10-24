use bstr::ByteSlice;
use csv::ByteRecord;
use regex::bytes::Regex;

use crate::config::{Config, Delimiter};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Separate columns into multiple columns by splitting cell values on a separator or regex.
By default, all possible splits are made, but you can limit the number of splits
using the --max-splitted-cells option.
Note that by default, the original columns are removed from the output. Use the --keep-column
flag to retain them.

This command takes the specified columns and splits each cell in those columns using either
a substring separator or a regex pattern. The resulting parts are output as new columns.
You can choose to split by a simple substring or use a regex for more complex splitting.
Additional options allow you to extract only matching parts, or capture groups from the regex.

Examples:

  Split column named 'fullname' on space:
    $ xan separate fullname ' ' data.csv

  Split column named 'fullname' on whitespaces using a regex:
    $ xan separate -r fullname '\s+' data.csv

  Extract digit sequences from column named 'birthdate' as separate columns using a regex:
    $ xan separate -r -m birthdate '\d+' data.csv

  Extract year, month and day from column named 'date' using capture groups:
    $ xan separate date '(\d{4})-(\d{2})-(\d{2})' data.csv -r -c --into year,month,day

  Split column 'code' into parts of fixed width 3:
    $ xan separate code --fixed-width 3 data.csv

  Split column 'code' into parts of widths 2,4,3:
    $ xan separate code --widths 2,4,3 data.csv

  Split column 'code' into parts of offsets 2,6,9 (same as widths 2,4,3):
    $ xan separate code --offsets 2,6,9 data.csv

Usage:
    xan separate [options] <columns> --fixed-width <width> [<input>]
    xan separate [options] <columns> --widths <widths> [<input>]
    xan separate [options] <columns> --offsets <offsets> [<input>]
    xan separate [options] <columns> <separator> [<input>]
    xan separate --help

separate options:
    -k, --keep               Keep the separated columns after splitting.
    --max-splitted-cells <n>
                             Limit the number of cells splitted to at most <n>.
                             By default, all possible splits are made.
    --into <column-names>    Specify names for the new columns created by the
                             splits. If not provided, new columns will be named
                             split1, split2, etc. If used with --max-splitted-cells,
                             the number of names provided must be equal or lower
                             than <n>.
    --too-many <option>      Specify how to handle extra cells when the number
                             of splitted cells exceeds --max-splitted-cells, or
                             the number of provided names with --into.
                             By default, it will cause an error. Options are 'drop'
                             to discard extra parts, or 'merge' to combine them
                             into the last column. Note that 'merge' cannot be
                             used with -m/--match nor -c/--capture-groups.
    -r, --regex              When using --separator, split cells using <separator>
                             as a regex pattern instead of splitting.
    -m, --match              When using -r/--regex, only output the parts of the
                             cell that match the regex pattern. By default, the
                             parts between matches (i.e. separators) are output.
    -c, --capture-groups     When using -r/--regex, if the regex contains capture
                             groups, output the text matching each capture group
                             as a separate column.
    --fixed-width <width>    Used without <separator>. Instead of splitting on a
                             separator, split cells every <width> bytes. Cannot
                             be used with --widths nor --offsets.
    --widths <widths>        Used without <separator>. Instead of splitting on a
                             separator, split cells using the specified fixed
                             widths (comma-separated list of integers). Cannot be
                             used with --fixed-width, --offsets nor
                             --max-splitted-cells.
    --offsets <offsets>      Used without <separator>. Instead of splitting on a
                             separator, split cells using the specified offsets
                             (comma-separated list of integers). Cannot be used
                             with --fixed-width, --widths nor --max-splitted-cells.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
"#;

#[derive(Deserialize, Debug)]
struct Args {
    arg_columns: SelectColumns,
    arg_separator: Option<String>,
    arg_input: Option<String>,
    flag_regex: bool,
    flag_match: bool,
    flag_capture_groups: bool,
    flag_keep: bool,
    flag_max_splitted_cells: Option<usize>,
    flag_into: Option<String>,
    flag_too_many: Option<TooManyMode>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_fixed_width: Option<usize>,
    flag_widths: Option<String>,
    flag_offsets: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
enum TooManyMode {
    #[default]
    Abort,
    Drop,
    Merge,
}

impl TooManyMode {
    fn requires_splitn(&self) -> bool {
        matches!(self, Self::Merge | Self::Drop)
    }
}

enum RegexMode {
    Split,
    Match,
    CaptureGroups,
}

enum Splitter {
    Substring(Vec<u8>),
    Regex(Regex, RegexMode),
    FixedWidth(usize),
    Widths(Vec<usize>),
    Offsets(Vec<usize>),
}

impl Splitter {
    fn count_splits<'a>(&self, cells: impl Iterator<Item = &'a [u8]>) -> usize {
        let mut count = 0;

        for cell in cells {
            count += match self {
                Self::Substring(sep) => cell.find_iter(sep).count() + 1,
                Self::Regex(pattern, mode) => match mode {
                    RegexMode::Split => pattern.find_iter(cell).count() + 1,
                    RegexMode::CaptureGroups => {
                        pattern.captures_iter(cell).map(|m| m.len() - 1).sum()
                    }
                    RegexMode::Match => pattern.find_iter(cell).count(),
                },
                Self::FixedWidth(width) => cell.len().div_ceil(*width),
                Self::Widths(widths) => widths.len(),
                Self::Offsets(offsets) => offsets.len(),
            };
        }

        count
    }

    fn split<'s, 'c>(&'s self, cell: &'c [u8]) -> Box<dyn Iterator<Item = &'c [u8]> + 's>
    where
        'c: 's,
    {
        match self {
            Self::Substring(sep) => Box::new(cell.split_str(sep)),
            Self::Regex(pattern, mode) => match mode {
                RegexMode::Split => Box::new(pattern.split(cell)),
                RegexMode::CaptureGroups => {
                    Box::new(pattern.captures_iter(cell).flat_map(|caps| {
                        caps.iter()
                            .skip(1)
                            .map(|m| m.map(|b| b.as_bytes()).unwrap_or(b""))
                            .collect::<Vec<_>>()
                    }))
                }
                RegexMode::Match => Box::new(pattern.find_iter(cell).map(|m| m.as_bytes())),
            },
            Self::FixedWidth(width) => Box::new(cell.chunks(*width).map(|chunk| chunk.trim())),
            Self::Widths(widths) => {
                let mut remaining = widths.len();
                let mut splitted = vec![];
                let mut start = 0;

                for width in widths.iter() {
                    if start + width <= cell.len() {
                        splitted.push(cell[start..start + width].trim());
                    } else {
                        splitted.push(cell[start..].trim());
                    }
                    start += width;
                    remaining -= 1;
                }
                while remaining > 0 {
                    splitted.push(b"");
                    remaining -= 1;
                }
                Box::new(splitted.into_iter())
            }
            Self::Offsets(offsets) => {
                let mut remaining = offsets.len();
                let mut splitted = vec![];
                let mut start = 0;

                for offset in offsets.iter() {
                    if *offset <= cell.len() {
                        splitted.push(cell[start..*offset].trim());
                    } else {
                        splitted.push(cell[start..].trim());
                    }
                    start = *offset;
                    remaining -= 1;
                }
                while remaining > 0 {
                    splitted.push(b"");
                    remaining -= 1;
                }
                Box::new(splitted.into_iter())
            }
        }
    }

    fn splitn<'s, 'c>(
        &'s self,
        limit: usize,
        cell: &'c [u8],
    ) -> Box<dyn Iterator<Item = &'c [u8]> + 's>
    where
        'c: 's,
    {
        match self {
            Self::Substring(sep) => Box::new(cell.splitn_str(limit, sep)),
            Self::Regex(pattern, mode) => match mode {
                RegexMode::Split => Box::new(pattern.splitn(cell, limit)),
                RegexMode::CaptureGroups => {
                    unimplemented!()
                }
                RegexMode::Match => unimplemented!(),
            },
            Self::FixedWidth(width) => Box::new(cell.chunks(*width).take(limit)),
            Self::Widths(_) => unimplemented!(),
            Self::Offsets(_) => unimplemented!(),
        }
    }

    fn split_cells<'a>(
        &self,
        cells: impl Iterator<Item = &'a [u8]>,
        max_splitted_cells: usize,
        too_many_mode: TooManyMode,
    ) -> CliResult<ByteRecord> {
        let mut output_record = ByteRecord::new();

        match too_many_mode {
            TooManyMode::Abort => {
                for cell in cells {
                    for sub_cell in self.split(cell) {
                        if output_record.len() == max_splitted_cells {
                            // TODO: we expected that much and got
                            Err(format!("Number of splits exceeded the given maximum: expected {}. Consider using the --too-many flag to handle extra splitted cells.", max_splitted_cells))?;
                        }

                        output_record.push_field(sub_cell);
                    }
                }
            }
            TooManyMode::Drop => {
                for sub_cell in cells
                    .flat_map(|cell| self.split(cell))
                    .take(max_splitted_cells)
                {
                    output_record.push_field(sub_cell);
                }
            }
            TooManyMode::Merge => {
                let mut remaining = max_splitted_cells;

                for cell in cells {
                    for sub_cell in self.splitn(remaining, cell) {
                        remaining -= 1;
                        output_record.push_field(sub_cell);
                    }
                }
            }
        };

        while output_record.len() < max_splitted_cells {
            output_record.push_field(b"");
        }

        Ok(output_record)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let splitters_count = args.flag_fixed_width.is_some() as u8
        + args.flag_widths.is_some() as u8
        + args.arg_separator.is_some() as u8
        + args.flag_offsets.is_some() as u8;

    if splitters_count == 0 {
        Err("One <separator>, --fixed-width, --widths or --offsets argument is required")?;
    } else if splitters_count > 1 {
        Err("Only one of <separator>, --fixed-width, --widths or --offsets argument can be used")?;
    }

    if args.flag_fixed_width.is_some() {
        if args.flag_regex || args.flag_capture_groups || args.flag_match {
            Err(
                "--fixed-width cannot be used with -r/--regex, -c/--capture-groups nor -m/--match",
            )?;
        }
    }

    let mut widths_or_offsets: Option<Vec<usize>> = None;

    if args.flag_widths.is_some() || args.flag_offsets.is_some() {
        if args.flag_regex || args.flag_capture_groups || args.flag_match {
            Err("--widths|--offsets cannot be used with -r/--regex, -c/--capture-groups nor -m/--match")?;
        }
        if args.flag_max_splitted_cells.is_some() {
            Err("--widths|--offsets cannot be used with --max-splitted-cells")?;
        }

        let widths_or_offsets_str = if let Some(widths) = args.flag_widths.clone() {
            widths
        } else {
            args.flag_offsets.clone().unwrap()
        };
        widths_or_offsets = Some(
            widths_or_offsets_str
                .split(',')
                .map(|s| s.trim().parse::<usize>())
                .collect::<Result<Vec<usize>, _>>()
                .map_err(|_| "Invalid value within --widths|--offsets.")?,
        );

        if args.flag_into.is_some()
            && util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len()
                > widths_or_offsets.clone().unwrap().len()
        {
            Err("--into cannot specify more column names than widths provided with --widths|--offsets")?;
        }
    }

    if args.flag_capture_groups || args.flag_match {
        if !args.flag_regex {
            Err("-c/--capture-groups and -m/--match can only be used with --regex")?;
        }
        if args.flag_capture_groups && args.flag_match {
            Err("-c/--capture-groups and -m/--match cannot be used together")?;
        }
    }

    let too_many_mode = args.flag_too_many.unwrap_or_default();

    if too_many_mode.requires_splitn() {
        if args.flag_max_splitted_cells.is_none() && args.flag_into.is_none() {
            Err("--too-many can only be used with --max-splitted-cells or --into")?;
        }

        if (args.flag_capture_groups || args.flag_match)
            && matches!(too_many_mode, TooManyMode::Merge)
        {
            Err("--too-many merge doesn't work with -c/--capture-groups nor -m/--match!")?;
        }
    }

    if args.flag_into.is_some()
        && args.flag_max_splitted_cells.is_some()
        && util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len()
            > args.flag_max_splitted_cells.unwrap()
    {
        Err(format!("--into cannot specify more column names than --max-splitted-cells : got {} for --into and {} for --max-splitted-cells", util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len(), args.flag_max_splitted_cells.unwrap()))?;
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_columns)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let sel = rconf.selection(&headers)?;

    let mut records: Vec<ByteRecord> = Vec::new();
    let mut max_splitted_cells: usize = 0;

    let splitter = if args.flag_regex {
        let regex_mode = if args.flag_match {
            RegexMode::Match
        } else if args.flag_capture_groups {
            RegexMode::CaptureGroups
        } else {
            RegexMode::Split
        };

        Splitter::Regex(Regex::new(&args.arg_separator.unwrap())?, regex_mode)
    } else if let Some(width) = args.flag_fixed_width {
        Splitter::FixedWidth(width)
    } else if let Some(widths_or_offsets) = widths_or_offsets {
        if args.flag_widths.is_some() {
            Splitter::Widths(widths_or_offsets)
        } else {
            Splitter::Offsets(widths_or_offsets)
        }
    } else {
        Splitter::Substring(args.arg_separator.unwrap().as_bytes().to_vec())
    };

    // When we need to determine the maximum number of splits across all rows
    // (to know how many new columns to create), we have to first read all records
    // and store them in memory.
    if let Some(n) = args.flag_max_splitted_cells {
        max_splitted_cells = n;
    } else if args.flag_into.is_some() {
        max_splitted_cells = util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len();
    } else {
        for result in rdr.byte_records() {
            let record = result?;

            let numsplits = splitter.count_splits(sel.select(&record));
            max_splitted_cells = max_splitted_cells.max(numsplits);

            records.push(record);
        }
    }

    let sel_to_keep: Selection = if args.flag_keep {
        Selection::full(headers.len())
    } else {
        sel.inverse(headers.len())
    };

    let mut new_headers: ByteRecord;
    if args.flag_keep {
        new_headers = headers.clone();
    } else {
        new_headers = sel_to_keep.select(&headers).collect::<ByteRecord>()
    }
    let mut number_of_new_columns = max_splitted_cells;
    if let Some(into) = &args.flag_into {
        let headers_to_add = util::str_to_csv_byte_record(into);
        new_headers.extend(&headers_to_add);
        number_of_new_columns -= headers_to_add.len();
    }

    for i in 1..=number_of_new_columns {
        let header_name = format!("split{}", i);
        new_headers.push_field(header_name.as_bytes());
    }

    wtr.write_byte_record(&new_headers)?;

    let mut process_record = |record: &ByteRecord| -> CliResult<()> {
        wtr.write_record(
            sel_to_keep.select(record).chain(
                splitter
                    .split_cells(sel.select(record), max_splitted_cells, too_many_mode)?
                    .iter(),
            ),
        )?;

        Ok(())
    };

    if args.flag_max_splitted_cells.is_some() || args.flag_into.is_some() {
        let mut record = ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            process_record(&record)?;
        }
    } else {
        for record in records {
            process_record(&record)?;
        }
    }

    Ok(wtr.flush()?)
}
