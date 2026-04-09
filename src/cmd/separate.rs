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
to some splitting method that can be one of:

    * (default): splitting by a single substring
    * -r, --regex: splitting using a regular expression
    * -m, --match: decomposing into regular expression matches
    * -c, --captures: decomposing into regular expression first match's capture groups
    * -C, --all-captures: decomposing into regular expression all matches' capture groups
    * --fixed-width: cutting every <n> bytes
    * --widths: split by a list of consecutive widths
    * --cuts: cut at predefined byte offsets
    * --offsets: extract byte slices

Created columns can be given a name using the --into flag, else they will be
given generic names based on the original column name. For instance, splitting a
column named "text" will produce columns named "text1", "text2"... The --prefix
flag can also be used to choose a different name.

Note that when using -c/--captures, column names can be deduced from regex capture
group names like in the following pattern: (?<year>\d{4})-(?<day>\d{2}).

It is also possible to limit the number of splits using the --max flag.

If the number of splits is known beforehand (that is to say when using --into
or --max, --widths, --cuts, --offsets or --captures), the command will be
able to stream the data. Else it will have to buffer the whole file into memory
to record the maximum number of splits produced by the selected method.

Finally, note that by default, the separated column will be removed from the output,
unless the -k/--keep flag is used.

Examples:

  Splitting a full name
    $ xan separate fullname ' ' data.csv
    $ xan separate --into first_name,last_name ' ' data.csv

  Splitting a full name using a regular expression
    $ xan separate -r fullname '\s+' data.csv

  Extracting digit sequences from a column named 'birthdate' using a regex:
    $ xan separate -rm birthdate '\d+' data.csv

  Extracting year, month and day from a column named 'date' using capture groups:
    $ xan separate -rc date '(\d{4})-(\d{2})-(\d{2})' data.csv --into year,month,day

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
    -r, --regex         Split cells using a regular expression instead of using
                        a simple substring.
    -m, --match         When using -r/--regex, extract parts of the cell matching
                        the regex pattern.
    -c, --captures      When using -r/--regex, find first match of given regex
                        pattern and extract its capture groups.
    -C, --all-captures  When using -r/--regex, find all matches of given regex
                        pattern and extract their capture groups.
    --fixed-width       Split cells every <separator> bytes.
    --widths            Split cells using the given widths (given as a comma-separated
                        list of integers).
    --cuts              Split cells on the given bytes (given as a comma-separated
                        list of increasing, non-repeating integers).
    --offsets           Split cells according to the specified byte offsets (given as a
                        comma-separated list of increasing, non-repeating integers).

separate options:
    -M, --max <n>          Limit the number of cells splitted to at most <n>.
                           By default, all possible splits are made.
    --into <column-names>  Specify names for the new columns created by the
                           splits. If not provided, new columns will be named
                           before the original column name ('text' column will
                           be separated into 'text1', 'text2', etc.). If used with --max,
                           the number of names provided must be equal or lower
                           than <n>. Cannot be used with --prefix.
    --prefix <prefix>      Specify a prefix for the new columns created by the
                           splits. By default, no prefix is used and new columns
                           are named before the original column name ('text'
                           column will be separated into 'text1', 'text2', etc.).
                           Cannot be used with --into.
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
                           nor -c/--captures.
                           [default: error]
    -k, --keep             Keep the separated column after splitting, instead of
                           discarding it.
    --trim                 Whether to trim splitted values of leading/trailing
                           whitespace.

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
    flag_all_captures: bool,
    flag_captures: bool,
    flag_keep: bool,
    flag_max: Option<usize>,
    flag_into: Option<String>,
    flag_prefix: Option<String>,
    flag_too_many: TooManyMode,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_fixed_width: bool,
    flag_widths: bool,
    flag_cuts: bool,
    flag_offsets: bool,
    flag_trim: bool,
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

#[derive(Debug, Clone, Copy)]
struct SplitOptions {
    max: usize,
    too_many_mode: TooManyMode,
    trim: bool,
}

#[derive(Debug)]
enum RegexMode {
    Split,
    Match,
    AllCaptures,
    Captures,
}

enum Split<'c, 'r> {
    Substring(bstr::Split<'c, 'r>),
    Regex(regex::bytes::Split<'r, 'c>),
    RegexMatches(regex::bytes::Matches<'r, 'c>),
    RegexAllCaptures {
        iter: regex::bytes::CaptureMatches<'r, 'c>,
        current: Option<regex::bytes::Captures<'c>>,
        group: usize,
    },
    RegexCaptures {
        caps: Option<regex::bytes::Captures<'c>>,
        group: usize,
    },
    FixedWidth(std::slice::Chunks<'c, u8>),
    Offsets {
        cell: &'c [u8],
        offsets: &'r [usize],
        index: usize,
        has_implicit_end: bool,
    },
}

impl<'c> Iterator for Split<'c, '_> {
    type Item = &'c [u8];

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Substring(iter) => iter.next(),
            Self::Regex(iter) => iter.next(),
            Self::RegexMatches(iter) => iter.next().map(|m| m.as_bytes()),
            Self::RegexAllCaptures {
                iter,
                current,
                group,
            } => {
                loop {
                    if let Some(caps) = current {
                        if *group < caps.len() {
                            let sub_cell = caps.get(*group).map(|m| m.as_bytes()).unwrap_or(b"");
                            *group += 1;
                            return Some(sub_cell);
                        }
                    }

                    match iter.next() {
                        Some(caps) => {
                            *current = Some(caps);
                            *group = 1; // skip full match
                        }
                        None => return None,
                    }
                }
            }
            Self::RegexCaptures { caps, group } => {
                let caps = caps.as_ref()?;

                if *group >= caps.len() {
                    return None;
                }

                let sub_cell = caps.get(*group).map(|m| m.as_bytes()).unwrap_or(b"");

                *group += 1;

                Some(sub_cell)
            }
            Self::FixedWidth(iter) => iter.next(),
            Self::Offsets {
                cell,
                offsets,
                index,
                has_implicit_end,
            } => {
                if *index + 1 < offsets.len() {
                    let start = offsets[*index];
                    let end = offsets[*index + 1].min(cell.len());

                    *index += 1;

                    if start >= cell.len() {
                        return None;
                    }

                    return Some(&cell[start..end]);
                }

                if *has_implicit_end && *index < offsets.len() {
                    let start = offsets[*index];
                    *index += 1;

                    if start < cell.len() {
                        return Some(&cell[start..]);
                    }
                }

                None
            }
        }
    }
}

enum SplitN<'c, 'r> {
    Substring(bstr::SplitN<'c, 'r>),
    Regex(regex::bytes::SplitN<'r, 'c>),
    FixedWidth(std::iter::Take<std::slice::Chunks<'c, u8>>),
}

impl<'c> Iterator for SplitN<'c, '_> {
    type Item = &'c [u8];

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Substring(iter) => iter.next(),
            Self::Regex(iter) => iter.next(),
            Self::FixedWidth(iter) => iter.next(),
        }
    }
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
            Self::Regex(regex, RegexMode::Captures) => Some(regex.capture_names().skip(1).count()),
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
                RegexMode::AllCaptures => pattern.captures_iter(cell).map(|m| m.len() - 1).sum(),
                RegexMode::Match => pattern.find_iter(cell).count(),
                RegexMode::Captures => unreachable!(),
            },
            Self::FixedWidth(width) => cell.len().div_ceil(*width),
            Self::Offsets(_, _) => unreachable!(),
        }
    }

    fn split<'c, 'r>(&'r self, cell: &'c [u8]) -> Split<'c, 'r> {
        match self {
            Self::Substring(sep) => Split::Substring(cell.split_str(sep)),
            Self::Regex(pattern, mode) => match mode {
                RegexMode::Split => Split::Regex(pattern.split(cell)),
                RegexMode::Match => Split::RegexMatches(pattern.find_iter(cell)),
                RegexMode::AllCaptures => Split::RegexAllCaptures {
                    iter: pattern.captures_iter(cell),
                    current: None,
                    group: 0,
                },
                RegexMode::Captures => Split::RegexCaptures {
                    caps: pattern.captures(cell),
                    group: 1,
                },
            },
            Self::FixedWidth(width) => Split::FixedWidth(cell.chunks(*width)),
            Self::Offsets(offsets, has_implicit_end) => Split::Offsets {
                cell,
                offsets,
                index: 0,
                has_implicit_end: *has_implicit_end,
            },
        }
    }

    fn splitn<'c, 'r>(&'r self, limit: usize, cell: &'c [u8]) -> SplitN<'c, 'r> {
        match self {
            Self::Substring(sep) => SplitN::Substring(cell.splitn_str(limit, sep)),
            Self::Regex(pattern, mode) => match mode {
                RegexMode::Split => SplitN::Regex(pattern.splitn(cell, limit)),
                RegexMode::AllCaptures => {
                    unreachable!()
                }
                RegexMode::Match => unreachable!(),
                RegexMode::Captures => unreachable!(),
            },
            Self::FixedWidth(width) => SplitN::FixedWidth(cell.chunks(*width).take(limit)),
            Self::Offsets(_, _) => unreachable!(),
        }
    }

    fn split_cell_into(
        &self,
        cell: &[u8],
        options: SplitOptions,
        record: &mut ByteRecord,
    ) -> CliResult<()> {
        let SplitOptions {
            max,
            too_many_mode,
            trim,
        } = options;

        record.clear();

        match too_many_mode {
            TooManyMode::Error => {
                for sub_cell in self.split(cell) {
                    record.push_field(if trim { sub_cell.trim() } else { sub_cell });
                }

                if record.len() > max {
                    Err(format!("Number of splits exceeded expected maximum {} but got {}. Consider using the --too-many flag to handle extra splitted cells.", max, record.len()))?;
                }
            }
            TooManyMode::Drop => {
                for sub_cell in self.split(cell).take(max) {
                    record.push_field(if trim { sub_cell.trim() } else { sub_cell });
                }
            }
            TooManyMode::Merge => {
                for sub_cell in self.splitn(max, cell) {
                    record.push_field(if trim { sub_cell.trim() } else { sub_cell });
                }
            }
        };

        // Padding
        while record.len() < max {
            record.push_field(b"");
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let segmenters_count = args.flag_fixed_width as u8
        + args.flag_widths as u8
        + args.flag_cuts as u8
        + args.flag_offsets as u8
        + args.flag_regex as u8;

    if segmenters_count > 1 {
        Err("Only one of -r/--regex, --fixed-width, --widths, --cuts or --offsets argument can be used!")?;
    }

    let regex_mode_count =
        args.flag_match as u8 + args.flag_captures as u8 + args.flag_all_captures as u8;

    if args.flag_regex {
        if regex_mode_count > 1 {
            Err("Only one of -m/--match, -c/--captures or -C/--all-captures can be used!")?;
        }
    } else if regex_mode_count > 0 {
        Err("-m/--match, -c/--captures and -C/--all-captures can only be used with -r/--regex!")?;
    }

    let too_many_mode = args.flag_too_many;

    if too_many_mode.requires_splitn() {
        if args.flag_max.is_none() && args.flag_into.is_none() {
            Err("--too-many can only be used with --max or --into!")?;
        }

        if (args.flag_captures || args.flag_all_captures || args.flag_match)
            && matches!(too_many_mode, TooManyMode::Merge)
        {
            Err("--too-many merge doesn't work with -c/--captures nor -m/--match!")?;
        }
    }

    let mut new_column_names = args
        .flag_into
        .as_ref()
        .map(|names| util::str_to_csv_byte_record(names));

    match (args.flag_max, &new_column_names, &args.flag_prefix) {
        (_, Some(_), Some(_)) => {
            Err("--into and --prefix cannot be used together!")?;
        }
        (Some(max), Some(names), _) if names.len() > max => {
            Err(format!("--into cannot specify more column names than --max : got {} for --into and {} for --max", names.len(), max))?;
        }
        _ => (),
    }

    let rconf = Config::new(&args.arg_input)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column.clone())
        .delimiter(args.flag_delimiter);

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let separated_column_index = rconf.single_selection(&headers)?;

    let prefix = args
        .flag_prefix
        .unwrap_or(String::from_utf8_lossy(&headers[separated_column_index]).into_owned());

    let splitter = if args.flag_regex {
        let pattern = Regex::new(&args.arg_separator)?;

        let regex_mode = if args.flag_match {
            RegexMode::Match
        } else if args.flag_captures {
            if new_column_names.is_none() {
                new_column_names = Some(
                    pattern
                        .capture_names()
                        .skip(1)
                        .enumerate()
                        .map(|(i, name_opt)| match name_opt {
                            Some(name) => name.to_string().into_bytes(),
                            None => format!("{}{}", prefix, i + 1).into_bytes(),
                        })
                        .collect(),
                );
            }

            RegexMode::Captures
        } else if args.flag_all_captures {
            RegexMode::AllCaptures
        } else {
            RegexMode::Split
        };

        let splitter = Splitter::Regex(pattern, regex_mode);

        if matches!((&new_column_names, splitter.static_count_splits()), (Some(names), Some(expected_count)) if names.len() != expected_count)
        {
            Err("--into cannot specify more column names than given regex capture groups when using -c/--captures!")?;
        }

        splitter
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
        let mut offset: usize = 0;

        if let Some(names) = &new_column_names {
            let to_extend = names.iter().take(number_of_new_columns);
            offset = to_extend.len();
            new_headers.extend(to_extend);
            number_of_new_columns = number_of_new_columns.saturating_sub(names.len());
        }

        for i in 1..=number_of_new_columns {
            let header_name = format!("{}{}", prefix, offset + i);
            new_headers.push_field(header_name.as_bytes());
        }

        new_headers.extend(headers.iter().skip(separated_column_index + 1));

        wtr.write_byte_record(&new_headers)?;
    }

    let split_options = SplitOptions {
        too_many_mode,
        max: max_splits,
        trim: args.flag_trim,
    };

    // Flushing
    let mut process_record =
        |record: &ByteRecord, output_record: &mut ByteRecord| -> CliResult<()> {
            splitter.split_cell_into(
                &record[separated_column_index],
                split_options,
                output_record,
            )?;

            wtr.write_record(
                record
                    .iter()
                    .take(separated_column_index + args.flag_keep as usize)
                    .chain(output_record.iter())
                    .chain(record.iter().skip(separated_column_index + 1)),
            )?;

            Ok(())
        };

    let mut output_record = ByteRecord::new();

    if let Some(records) = buffered_records {
        for record in records {
            process_record(&record, &mut output_record)?;
        }
    } else {
        let mut record = ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            process_record(&record, &mut output_record)?;
        }
    }

    Ok(wtr.flush()?)
}
