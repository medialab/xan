use bstr::ByteSlice;
use regex::bytes::Regex;
use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Separate ONE column into multiple columns by splitting cell values on a separator or regex.
By default, all possible splits are made, but you can limit the number of splits
using the --max-splitted-cells option.
Note that by default, the original column is removed from the output. Use the --keep-column
flag to retain it.

This command takes the specified column and splits each cell in that column using either
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

  Split column 'code' on bytes 2,6:
    $ xan separate code --split-on-bytes 2,6 data.csv

  Split column 'code' into parts of segments defined by offsets 0,2,6,9 (same as
  split-on-bytes 2,6 if the length of the cell is 9):
    $ xan separate code --segment-bytes 0,2,6,9 data.csv

Usage:
    xan separate [options] <column> <separator> [<input>]
    xan separate --help

separate options:
    -k, --keep                Keep the separated column after splitting.
    --max-splitted-cells <n>  Limit the number of cells splitted to at most <n>.
                              By default, all possible splits are made.
    --into <column-names>     Specify names for the new columns created by the
                              splits. If not provided, new columns will be named
                              split1, split2, etc. If used with --max-splitted-cells,
                              the number of names provided must be equal or lower
                              than <n>.
    --too-many <option>       Specify how to handle extra cells when the number
                              of splitted cells exceeds --max-splitted-cells, or
                              the number of provided names with --into.
                              By default, it will cause an error. Options are 'drop'
                              to discard extra parts, or 'merge' to combine them
                              into the last column. Note that 'merge' cannot be
                              used with -m/--match nor -c/--capture-groups.
                              [default: error]
    -r, --regex               When using --separator, split cells using <separator>
                              as a regex pattern instead of splitting.
    -m, --match               When using -r/--regex, only output the parts of the
                              cell that match the regex pattern. By default, the
                              parts between matches (i.e. separators) are output.
    -c, --capture-groups      When using -r/--regex, if the regex contains capture
                              groups, output the text matching each capture group
                              as a separate column.
    --fixed-width             Split cells every <separator> bytes. Cannot be used
                              with --widths, --split-on-bytes nor --segment-bytes.
                              Trims whitespace for each splitted cell.
    --widths                  Split cells using the specified fixed widths
                              (comma-separated list of integers). Cannot be
                              used with --fixed-width, --split-on-bytes, --segment-bytes
                              nor --max-splitted-cells. Trims whitespace for each
                              splitted cell.
    --split-on-bytes          Split cells on the specified bytes
                              (comma-separated list of integers). Cannot be used
                              with --fixed-width, --widths, --segment-bytes
                              nor --max-splitted-cells. Trims whitespace for each
                              splitted cell.
    --segment-bytes           Split cells according to the specified byte offsets
                              (comma-separated list of integers). Cannot be used
                              with --fixed-width, --widths, --split-on-bytes
                              nor --max-splitted-cells. Trims whitespace for
                              each splitted cell. When the first byte is 0 and
                              the last byte is equal to the cell length,
                              this is equivalent to --split-on-bytes (we're being
                              more explicit here).

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
    arg_column: SelectedColumns,
    arg_separator: String,
    arg_input: Option<String>,
    flag_regex: bool,
    flag_match: bool,
    flag_capture_groups: bool,
    flag_keep: bool,
    flag_max_splitted_cells: Option<usize>,
    flag_into: Option<String>,
    flag_too_many: TooManyMode,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_fixed_width: bool,
    flag_widths: bool,
    flag_split_on_bytes: bool,
    flag_segment_bytes: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
enum TooManyMode {
    Error,
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
    SegmentBytes(Vec<u64>, bool),
}

impl Splitter {
    fn static_count_splits(&self) -> Option<usize> {
        match self {
            Self::SegmentBytes(offsets, implicit_final_byte) => {
                Some(offsets.len() - (if *implicit_final_byte { 0 } else { 1 }))
            }
            _ => None,
        }
    }

    fn count_splits(&self, cell: &[u8]) -> usize {
        match self {
            Self::Substring(sep) => cell.find_iter(sep).count() + 1,
            Self::Regex(pattern, mode) => match mode {
                RegexMode::Split => pattern.find_iter(cell).count() + 1,
                RegexMode::CaptureGroups => pattern.captures_iter(cell).map(|m| m.len() - 1).sum(),
                RegexMode::Match => pattern.find_iter(cell).count(),
            },
            Self::FixedWidth(width) => cell.len().div_ceil(*width),
            Self::SegmentBytes(offsets, implicit_final_byte) => {
                if *implicit_final_byte {
                    offsets.len()
                } else {
                    offsets.len() - 1
                }
            }
        }
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
            Self::SegmentBytes(offsets, implicit_final_byte) => {
                let mut remaining = self.count_splits(cell);
                let mut splitted = vec![];
                let mut start = offsets[0] as usize;

                for offset in offsets.iter().skip(1) {
                    if *offset <= cell.len() as u64 {
                        splitted.push(cell[start..*offset as usize].trim());
                    } else {
                        splitted.push(cell[start..].trim());
                        break;
                    }
                    start = *offset as usize;
                    remaining -= 1;
                }

                // Add the final segment if the final byte is implicit,
                // i.e. corresponds to the end of the cell.
                if *implicit_final_byte && start < cell.len() {
                    splitted.push(cell[start..].trim());
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
            Self::SegmentBytes(_, _) => unimplemented!(),
        }
    }

    fn split_cell(
        &self,
        cell: &[u8],
        max_splitted_cells: usize,
        too_many_mode: TooManyMode,
    ) -> CliResult<ByteRecord> {
        let mut output_record = ByteRecord::new();

        match too_many_mode {
            TooManyMode::Error => {
                for sub_cell in self.split(cell) {
                    if output_record.len() == max_splitted_cells {
                        // TODO: we expected that much and got
                        Err(format!("Number of splits exceeded the given maximum: expected {}. Consider using the --too-many flag to handle extra splitted cells.", max_splitted_cells))?;
                    }

                    output_record.push_field(sub_cell);
                }
            }
            TooManyMode::Drop => {
                for sub_cell in self.split(cell).take(max_splitted_cells) {
                    output_record.push_field(sub_cell);
                }
            }
            TooManyMode::Merge => {
                let mut remaining = max_splitted_cells;

                for sub_cell in self.splitn(remaining, cell) {
                    remaining -= 1;
                    output_record.push_field(sub_cell);
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

    let segments_count = args.flag_fixed_width as u8
        + args.flag_widths as u8
        + args.flag_split_on_bytes as u8
        + args.flag_segment_bytes as u8;

    if segments_count > 1 {
        Err("Only one of --fixed-width, --widths, --split-on-bytes or --segment-bytes argument can be used")?;
    }

    if args.flag_fixed_width && (args.flag_regex || args.flag_capture_groups || args.flag_match) {
        Err("--fixed-width cannot be used with -r/--regex, -c/--capture-groups nor -m/--match")?;
    }
    if args.flag_widths || args.flag_split_on_bytes || args.flag_segment_bytes {
        if args.flag_regex || args.flag_capture_groups || args.flag_match {
            Err("--widths|--split-on-bytes|--segment-bytes cannot be used with -r/--regex, -c/--capture-groups nor -m/--match")?;
        } else if args.flag_max_splitted_cells.is_some() {
            Err("--widths|--split-on-bytes|--segment-bytes cannot be used with --max-splitted-cells")?;
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

    let too_many_mode = args.flag_too_many;

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
        .select(args.arg_column)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let separated_column_index = rconf.single_selection(&headers)?;

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

        Splitter::Regex(Regex::new(&args.arg_separator)?, regex_mode)
    } else if args.flag_fixed_width {
        Splitter::FixedWidth(
            args.arg_separator
                .parse::<usize>()
                .map_err(|_| "Invalid value for --fixed-width. It must be a positive integer.")?,
        )
    } else if args.flag_widths || args.flag_split_on_bytes || args.flag_segment_bytes {
        let widths_or_offsets: Vec<u64> = args
            .arg_separator
            .split(',')
            .map(|s| s.trim().parse::<u64>())
            .collect::<Result<Vec<u64>, _>>()
            .map_err(|_| "Invalid value within --widths|--offsets.")?;

        if args.flag_into.is_some()
            && ((args.flag_split_on_bytes
                && util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len()
                    > widths_or_offsets.clone().len() + 1)
                || (args.flag_segment_bytes
                    && util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len()
                        > widths_or_offsets.clone().len() - 1))
        {
            Err("--into cannot specify more column names than widths provided with --widths|--split-on-bytes|--segment-bytes")?;
        }
        if args.flag_widths {
            let mut offsets: Vec<u64> = vec![0];
            let mut cumulative = 0;
            for width in widths_or_offsets.iter() {
                cumulative += *width;
                offsets.push(cumulative);
            }
            Splitter::SegmentBytes(offsets, false)
        } else if args.flag_split_on_bytes {
            Splitter::SegmentBytes(vec![0].into_iter().chain(widths_or_offsets).collect(), true)
        } else if widths_or_offsets.len() < 2 {
            Err("--segment-bytes requires at least two byte offsets")?
        } else {
            Splitter::SegmentBytes(widths_or_offsets, false)
        }
    } else {
        Splitter::Substring(args.arg_separator.as_bytes().to_vec())
    };

    // When we need to determine the maximum number of splits across all rows
    // (to know how many new columns to create), we have to first read all records
    // and store them in memory.
    if let Some(n) = args.flag_max_splitted_cells {
        max_splitted_cells = n;
    } else if args.flag_into.is_some() {
        max_splitted_cells = util::str_to_csv_byte_record(&args.flag_into.clone().unwrap()).len();
    } else if let Some(n) = splitter.static_count_splits() {
        max_splitted_cells = n;
    } else {
        for result in rdr.byte_records() {
            let record = result?;

            let numsplits = splitter.count_splits(&record[separated_column_index]);
            max_splitted_cells = max_splitted_cells.max(numsplits);

            records.push(record);
        }
    }

    if !rconf.no_headers {
        let mut new_headers = ByteRecord::new();
        new_headers.extend(
            headers
                .iter()
                .take(separated_column_index + args.flag_keep as usize),
        );

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

        new_headers.extend(headers.iter().skip(separated_column_index + 1));

        wtr.write_byte_record(&new_headers)?;
    }

    let mut process_record = |record: &ByteRecord| -> CliResult<()> {
        let splitted = splitter.split_cell(
            &record[separated_column_index],
            max_splitted_cells,
            too_many_mode,
        )?;

        wtr.write_record(
            record
                .iter()
                .take(separated_column_index + args.flag_keep as usize)
                .chain(splitted.iter())
                .chain(record.iter().skip(separated_column_index + 1)),
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
