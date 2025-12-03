use std::iter::once;
use std::num::NonZeroUsize;

use bstr::ByteSlice;
use regex::bytes::Regex;
use simd_csv::ByteRecord;

use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = r#"
Separate a single column into multiple ones by splitting its cells according
to some splitting algorithm that can be one of:

    * (default): splitting by a single substring
    * -r, --regex: splitting using a regular expression
    * -m, --match: decomposing into regular expression matches
    * -c, --capture-groups: decomposing into regular expression capture groups
    * --fixed-width: cutting every <n> bytes
    * --widths: split by a list of consecutive widths
    * --cuts: cut at predefined byte offsets
    * --offsets: extract byte slices

Created columns can be given a name using the --into flag, else they will be
given generic names like "split1",  "split2" and so forth.

It is also possible to limit the number of splits using the --max flag.

If the number of splits is known beforehand (that is to say when using --into
or --max or --widths or --cuts or --offsets), the command will be able to stream
the data. Else it will have to buffer the whole file into memory to record the
maximum number of splits produced by the selected method.

Finally, note that by default, the separated column will be removed from the output,
unless the -k/--keep flag is used.

Examples:

  Splitting a full name
    $ xan separate fullname ' ' data.csv
    $ xan separate --into first_name,last_name ' ' data.csv

  Splitting a full name using a regular expression
    $ xan separate -r fullname '\s+' data.csv

  Extracting digit sequences from a column named 'birthdate' using a regex:
    $ xan separate -r -m birthdate '\d+' data.csv

  Extracting year, month and day from a column named 'date' using capture groups:
    $ xan separate date '(\d{4})-(\d{2})-(\d{2})' data.csv -r -c --into year,month,day

  Splitting a column named 'code' into sequences of 3 bytes:
    $ xan separate code --fixed-width 3 data.csv

  Splitting a column named 'code' into parts of widths 2, 4 and 3:
    $ xan separate code --widths 2,4,3 data.csv

  Splitting a column named 'code' on bytes 2 and 6:
    $ xan separate code --cuts 2,6 data.csv

  Split column named 'code' into of segments defined by byte offsets [0, 2), [2, 6) and [6, 9):
    $ xan separate code --offsets 0,2,6,9 data.csv

Usage:
    xan separate [options] <column> <separator> [<input>]
    xan separate --help

separate mode options:
    -r, --regex           When using --separator, split cells using a regular
                          expression instead of a simple substring.
    -m, --match           When using -r/--regex, extract parts of the cell matching
                          the regex pattern.
    -c, --capture-groups  When using -r/--regex, extract parts of the call matching
                          the regex pattern's capture groups.
    --fixed-width         Split cells every <separator> bytes. Each resulting part
                          will then be trimmed of leading/trailing whitespace.
    --widths              Split cells using the given widths (given as a comma-separated
                          list of integers). Each resulting part will then be trimmed of
                          leading/trailing whitespace.
    --cuts                Split cells on the given bytes (given as a comma-separated
                          list of increasing, non-repeating integers). Each resulting part
                          will then be trimmed of leading/trailing whitespace.
    --offsets             Split cells according to the specified byte offsets (given as a
                          comma-separated list of increasing, non-repeating integers).
                          Each resulting part will then be trimmed of leading/trailing whitespace.

separate options:
    -M, --max <n>          Limit the number of cells splitted to at most <n>.
                           By default, all possible splits are made.
    --into <column-names>  Specify names for the new columns created by the
                           splits. If not provided, new columns will be named
                           split1, split2, etc. If used with --max,
                           the number of names provided must be equal or lower
                           than <n>.
    --too-many <option>    Specify how to handle extra cells when the number
                           of splitted cells exceeds --max, or
                           the number of provided names with --into.
                           Must be one of:
                                - 'error': stop as soon as an inconsistent number
                                    of splits is produced.
                                - 'drop': drop splits over expected maximum.
                                - 'merge': append the rest of the cell to the last
                                    produced split.
                           Note that 'merge' cannot be used with -m/--match
                           nor -c/--capture-groups.
                           [default: error]
    -k, --keep             Keep the separated column after splitting, instead of
                           discarding it.

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
    flag_max: Option<usize>,
    flag_into: Option<String>,
    flag_too_many: TooManyMode,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_fixed_width: bool,
    flag_widths: bool,
    flag_cuts: bool,
    flag_offsets: bool,
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

#[derive(Debug)]
enum RegexMode {
    Split,
    Match,
    CaptureGroups,
}

#[derive(Debug)]
enum Splitter {
    Substring(Vec<u8>),
    Regex(Regex, RegexMode),
    FixedWidth(usize),
    Offsets(Vec<usize>, bool),
}

impl Splitter {
    fn static_count_splits(&self) -> Option<usize> {
        match self {
            Self::Offsets(offsets, has_implicit_end) => {
                Some(offsets.len() - (if *has_implicit_end { 0 } else { 1 }))
            }
            _ => None,
        }
    }

    fn as_offsets(&self) -> Option<&[usize]> {
        match self {
            Self::Offsets(offsets, _) => Some(offsets),
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
            Self::Offsets(_, _) => unreachable!(),
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
            Self::Offsets(offsets, has_implicit_end) => {
                let mut splits = Vec::<&[u8]>::with_capacity(
                    offsets.len() - (if *has_implicit_end { 0 } else { 1 }),
                );

                let mut must_add_implicit_end = *has_implicit_end;

                for window in offsets.windows(2) {
                    let start = window[0];
                    let end = window[1].min(cell.len());

                    if start >= cell.len() {
                        must_add_implicit_end = false;
                        break;
                    }

                    splits.push(cell[start..end].trim());
                }

                if must_add_implicit_end {
                    let start = *offsets.last().unwrap();

                    if start < cell.len() {
                        splits.push(cell[start..].trim())
                    }
                }

                Box::new(splits.into_iter())
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
            Self::Offsets(_, _) => unimplemented!(),
        }
    }

    fn split_cell(
        &self,
        cell: &[u8],
        max: usize,
        too_many_mode: TooManyMode,
    ) -> CliResult<ByteRecord> {
        let mut output_record = ByteRecord::new();

        match too_many_mode {
            TooManyMode::Error => {
                for sub_cell in self.split(cell) {
                    output_record.push_field(sub_cell);
                }

                if output_record.len() > max {
                    Err(format!("Number of splits exceeded expected maximum {} but got {}. Consider using the --too-many flag to handle extra splitted cells.", max, output_record.len()))?;
                }
            }
            TooManyMode::Drop => {
                for sub_cell in self.split(cell).take(max) {
                    output_record.push_field(sub_cell);
                }
            }
            TooManyMode::Merge => {
                let mut remaining = max;

                for sub_cell in self.splitn(remaining, cell) {
                    remaining -= 1;
                    output_record.push_field(sub_cell);
                }
            }
        };

        // Padding
        while output_record.len() < max {
            output_record.push_field(b"");
        }

        Ok(output_record)
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let segmenters_count = args.flag_fixed_width as u8
        + args.flag_widths as u8
        + args.flag_cuts as u8
        + args.flag_offsets as u8;

    if segmenters_count > 1 {
        Err("Only one of --fixed-width, --widths, --cuts or --offsets argument can be used!")?;
    }

    if args.flag_fixed_width && (args.flag_regex || args.flag_capture_groups || args.flag_match) {
        Err("--fixed-width cannot be used with -r/--regex, -c/--capture-groups nor -m/--match!")?;
    }
    if args.flag_widths || args.flag_cuts || args.flag_offsets {
        if args.flag_regex || args.flag_capture_groups || args.flag_match {
            Err("--width, --cuts, --offsets cannot be used with -r/--regex, -c/--capture-groups nor -m/--match!")?;
        } else if args.flag_max.is_some() {
            Err("--widths, --cuts, --offsets cannot be used with --max!")?;
        }
    }

    if args.flag_capture_groups || args.flag_match {
        if !args.flag_regex {
            Err("-c/--capture-groups and -m/--match can only be used with --regex!")?;
        }
        if args.flag_capture_groups && args.flag_match {
            Err("-c/--capture-groups and -m/--match cannot be used together!")?;
        }
    }

    let too_many_mode = args.flag_too_many;

    if too_many_mode.requires_splitn() {
        if args.flag_max.is_none() && args.flag_into.is_none() {
            Err("--too-many can only be used with --max or --into!")?;
        }

        if (args.flag_capture_groups || args.flag_match)
            && matches!(too_many_mode, TooManyMode::Merge)
        {
            Err("--too-many merge doesn't work with -c/--capture-groups nor -m/--match!")?;
        }
    }

    let new_column_names = args
        .flag_into
        .as_ref()
        .map(|names| util::str_to_csv_byte_record(names));

    match (args.flag_max, &new_column_names) {
        (Some(max), Some(names)) if names.len() > max => {
            Err(format!("--into cannot specify more column names than --max : got {} for --into and {} for --max", names.len(), max))?;
        }
        _ => (),
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column)
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let separated_column_index = rconf.single_selection(&headers)?;

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
                .parse::<NonZeroUsize>()
                .map(NonZeroUsize::get)
                .map_err(|_| "Invalid value for --fixed-width. It must be a positive integer!")?,
        )
    } else if args.flag_widths || args.flag_cuts || args.flag_offsets {
        let widths_or_offsets: Vec<usize> = args
            .arg_separator
            .split(',')
            .map(|s| s.trim().parse::<usize>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Invalid values given to --widths, --offsets or --cuts!")?;

        let splitter = if args.flag_widths {
            let mut offsets: Vec<usize> = vec![0];
            let mut cumulative = 0;
            for width in widths_or_offsets.iter() {
                cumulative += *width;
                offsets.push(cumulative);
            }

            Splitter::Offsets(offsets, false)
        } else if args.flag_cuts {
            Splitter::Offsets(once(0).chain(widths_or_offsets).collect(), true)
        } else if widths_or_offsets.len() < 2 {
            Err("--offsets requires at least two byte offsets!")?
        } else {
            Splitter::Offsets(widths_or_offsets, false)
        };

        if let Some(names) = &new_column_names {
            if names.len() > splitter.static_count_splits().unwrap() {
                Err("--into cannot specify more column names than implied by --widths, --cuts or --offsets!")?;
            }
        }

        let mut offsets = splitter.as_offsets().unwrap().to_vec();

        if !offsets.is_sorted() {
            Err(format!("values given to --cuts or --offsets should be monotonically increasing but got {}!", &args.arg_separator))?;
        }

        let len_before = offsets.len();
        offsets.dedup();

        if len_before != offsets.len() {
            Err(format!(
                "values given to --cuts or --offsets should not be repeated but got {}!",
                &args.arg_separator
            ))?;
        }

        splitter
    } else {
        Splitter::Substring(args.arg_separator.as_bytes().to_vec())
    };

    let mut buffered_records: Option<Vec<ByteRecord>> = None;

    let max_splits = if let Some(n) = args.flag_max {
        n
    } else if let Some(names) = &new_column_names {
        names.len()
    } else if let Some(n) = splitter.static_count_splits() {
        n
    } else {
        // We need to buffer the records to memory to know what the max number
        // of splits is.
        let mut max_seen = 0;
        let mut records = Vec::new();

        for result in rdr.byte_records() {
            let record = result?;

            let numsplits = splitter.count_splits(&record[separated_column_index]);
            max_seen = max_seen.max(numsplits);

            records.push(record);
        }

        buffered_records = Some(records);

        max_seen
    };

    // Writing headers
    if !rconf.no_headers {
        let mut new_headers = ByteRecord::new();
        new_headers.extend(
            headers
                .iter()
                .take(separated_column_index + args.flag_keep as usize),
        );

        let mut number_of_new_columns = max_splits;

        if let Some(names) = &new_column_names {
            new_headers.extend(names);
            number_of_new_columns -= names.len();
        }

        for i in 1..=number_of_new_columns {
            let header_name = format!("split{}", i);
            new_headers.push_field(header_name.as_bytes());
        }

        new_headers.extend(headers.iter().skip(separated_column_index + 1));

        wtr.write_byte_record(&new_headers)?;
    }

    // Flushing
    let mut process_record = |record: &ByteRecord| -> CliResult<()> {
        let splitted =
            splitter.split_cell(&record[separated_column_index], max_splits, too_many_mode)?;

        wtr.write_record(
            record
                .iter()
                .take(separated_column_index + args.flag_keep as usize)
                .chain(splitted.iter())
                .chain(record.iter().skip(separated_column_index + 1)),
        )?;

        Ok(())
    };

    if let Some(records) = buffered_records {
        for record in records {
            process_record(&record)?;
        }
    } else {
        let mut record = ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            process_record(&record)?;
        }
    }

    Ok(wtr.flush()?)
}
