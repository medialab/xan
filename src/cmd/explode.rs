use csv;

use config::{Config, Delimiter};
use select::SelectColumns;
use util;
use CliError;
use CliResult;

// NOTE: shamelessly stolen from:
// https://stackoverflow.com/questions/38821671/how-can-slices-be-split-using-another-slice-as-a-delimiter
struct SplitSubsequence<'a, 'b, T: 'a + 'b> {
    slice: &'a [T],
    needle: &'b [T],
    ended: bool,
}

impl<'a, 'b, T: 'a + 'b + PartialEq> Iterator for SplitSubsequence<'a, 'b, T> {
    type Item = &'a [T];

    fn next(&mut self) -> Option<Self::Item> {
        if self.ended {
            None
        } else if self.slice.is_empty() {
            self.ended = true;
            Some(self.slice)
        } else if let Some(p) = self
            .slice
            .windows(self.needle.len())
            .position(|w| w == self.needle)
        {
            let item = &self.slice[..p];
            self.slice = &self.slice[p + self.needle.len()..];
            Some(item)
        } else {
            self.ended = true;
            let item = self.slice;
            self.slice = &self.slice[self.slice.len() - 1..];
            Some(item)
        }
    }
}

fn split_subsequence<'a, 'b, T>(slice: &'a [T], needle: &'b [T]) -> SplitSubsequence<'a, 'b, T>
where
    T: 'a + 'b + PartialEq,
{
    SplitSubsequence {
        slice,
        needle,
        ended: false,
    }
}

static USAGE: &str = "
Explodes a row into multiple ones by splitting column values by using the
provided separator.

This is the reverse of the 'implode' command.

For instance the following CSV:

name,colors
John,blue|yellow
Mary,red

Can be exploded on the \"colors\" column using the \"|\" <separator> to produce:

name,colors
John,blue
John,yellow
Mary,red

Note finally that the file can be exploded on multiple well-aligned columns.

Usage:
    xan explode [options] <columns> <separator> [<input>]
    xan explode --help

explode options:
    -r, --rename <name>    New names for the exploded columns. Must be written
                           in CSV format if exploding multiple columns.
                           See 'xsv rename' help for more details.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
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
    let single_byte_sep = if sep.len() == 1 { Some(&sep[0]) } else { None };

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        let splits: Vec<Vec<&[u8]>> = sel
            .select(&record)
            .map(|cell| {
                if let Some(s) = single_byte_sep {
                    cell.split(|b| b == s).collect()
                } else {
                    split_subsequence(cell, sep).collect()
                }
            })
            .collect();

        if splits.iter().skip(1).any(|s| s.len() != splits[0].len()) {
            return Err(CliError::Other(
                "inconsistent exploded length accross columns.".to_string(),
            ));
        }

        if splits[0].is_empty() {
            wtr.write_byte_record(&record)?;
            continue;
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
