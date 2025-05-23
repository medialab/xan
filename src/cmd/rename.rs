use lazy_static::lazy_static;
use regex::bytes::Regex as BytesRegex;
use regex::Regex;
use unidecode::unidecode;

use crate::config::{Config, Delimiter};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

lazy_static! {
    static ref TO_UNDERSCORE_REGEX: Regex = Regex::new(r"[\s\-]").unwrap();
    static ref TO_DROP_REGEX: Regex = Regex::new(r#"[^A-Za-z0-9_]"#).unwrap();
}

fn slugify(name: &[u8]) -> String {
    let name = std::str::from_utf8(name).expect("invalid utf-8");
    let name = TO_UNDERSCORE_REGEX.replace_all(name, "_");
    let name = unidecode(&name);
    let name = TO_DROP_REGEX.replace_all(&name, "");

    name.into_owned()
}

static USAGE: &str = "
Rename columns of a CSV file. Can also be used to add headers to a headless
CSV file. The new names must be passed in CSV format to the column as argument,
which can be useful if the desired column names contains actual commas and/or double
quotes.

Renaming all columns:

    $ xan rename NAME,SURNAME,AGE file.csv

Renaming a selection of columns:

    $ xan rename NAME,SURNAME -s name,surname file.csv
    $ xan rename NAME,SURNAME -s '0-1' file.csv

Adding a header to a headless file:

    $ xan rename -n name,surname file.csv

Prefixing column names:

    $ xan rename --prefix university_ file.csv

Column names with characters that need escaping:

    $ xan rename 'NAME OF PERSON,\"AGE, \"\"OF\"\" PERSON\"' file.csv

Usage:
    xan rename [options] --replace <pattern> <replacement> [<input>]
    xan rename [options] --prefix <prefix> [<input>]
    xan rename [options] --suffix <suffix> [<input>]
    xan rename [options] --slugify [<input>]
    xan rename [options] <columns> [<input>]
    xan rename --help

rename options:
    -s, --select <arg>     Select the columns to rename. See 'xan select -h'
                           for the full syntax. Note that given selection must
                           not include a same column more than once.
    -p, --prefix <prefix>  Prefix to add to all column names.
    -x, --suffix <suffix>  Suffix to add to all column names.
    -S, --slugify          Transform the column name so that they are safe to
                           be used as identifiers. Will typically replace
                           whitespace & dashes with underscores, drop accentuation
                           etc.
    -R, --replace          Replace matches of a pattern by given replacement in
                           column names.
    -f, --force            Ignore unknown columns to be renamed.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_columns: Option<String>,
    arg_pattern: Option<String>,
    arg_replacement: Option<String>,
    flag_select: Option<SelectColumns>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_prefix: Option<String>,
    flag_suffix: Option<String>,
    flag_slugify: bool,
    flag_replace: bool,
    flag_force: bool,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let mut rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = rconfig.reader()?;

    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut record = csv::ByteRecord::new();

    if args.flag_no_headers {
        if args.flag_prefix.is_some() {
            Err("Cannot use -p/--prefix with --no-headers!")?;
        }

        if args.flag_suffix.is_some() {
            Err("Cannot use -x/--suffix with --no-headers!")?;
        }

        if args.flag_slugify {
            Err("Cannot use -S/--slugify with -n/--no-headers!")?;
        }

        if args.flag_replace {
            Err("Cannot use -R/--replace with -n/--no-headers!")?;
        }

        let rename_as = util::str_to_csv_byte_record(&args.arg_columns.unwrap());

        let expected_len = if rdr.read_byte_record(&mut record)? {
            record.len()
        } else {
            0
        };

        if expected_len != rename_as.len() {
            Err(format!(
                "Renamed columns alignement error. Expected {} names and got {}.",
                expected_len,
                rename_as.len(),
            ))?;
        }

        if expected_len > 0 {
            wtr.write_byte_record(&rename_as)?;
            wtr.write_byte_record(&record)?;

            while rdr.read_byte_record(&mut record)? {
                wtr.write_byte_record(&record)?;
            }
        }

        return Ok(wtr.flush()?);
    }

    let headers = rdr.byte_headers()?;
    let mut ignored_in_rename_as: Option<Vec<usize>> = None;
    let mut nothing_to_do = false;

    let selection = match args.flag_select {
        Some(mut selection) => {
            if args.flag_force {
                ignored_in_rename_as = Some(selection.retain_known(headers));

                if selection.is_empty() {
                    nothing_to_do = true;
                }
            }

            rconfig = rconfig.select(selection);
            rconfig.selection(headers)?
        }
        None => Selection::full(headers.len()),
    };

    if selection.has_duplicates() {
        Err("Cannot rename a column selection where some columns appear multiple times!")?;
    }

    let renamed_headers: csv::ByteRecord = if nothing_to_do {
        headers.clone()
    } else if args.flag_slugify {
        headers
            .iter()
            .zip(selection.mask(headers.len()))
            .map(|(h, is_selected)| {
                if is_selected {
                    slugify(h).into_bytes()
                } else {
                    h.to_vec()
                }
            })
            .collect()
    } else if let Some(prefix) = args.flag_prefix {
        headers
            .iter()
            .zip(selection.mask(headers.len()))
            .map(|(h, is_selected)| {
                if is_selected {
                    [prefix.as_bytes(), h].concat()
                } else {
                    h.to_vec()
                }
            })
            .collect()
    } else if let Some(suffix) = args.flag_suffix {
        headers
            .iter()
            .zip(selection.mask(headers.len()))
            .map(|(h, is_selected)| {
                if is_selected {
                    [h, suffix.as_bytes()].concat()
                } else {
                    h.to_vec()
                }
            })
            .collect()
    } else if args.flag_replace {
        let pattern = BytesRegex::new(&args.arg_pattern.unwrap())?;
        let replacement = args.arg_replacement.unwrap();

        headers
            .iter()
            .zip(selection.mask(headers.len()))
            .map(|(h, is_selected)| {
                if is_selected {
                    pattern.replace_all(h, replacement.as_bytes()).into_owned()
                } else {
                    h.to_vec()
                }
            })
            .collect()
    } else {
        let mut rename_as = util::str_to_csv_byte_record(&args.arg_columns.unwrap());

        if let Some(ignored) = ignored_in_rename_as {
            rename_as = rename_as
                .into_iter()
                .enumerate()
                .filter_map(|(i, c)| if ignored.contains(&i) { None } else { Some(c) })
                .collect::<csv::ByteRecord>();
        }

        if selection.len() != rename_as.len() {
            Err(format!(
                "Renamed columns alignement error. Expected {} names and got {}.",
                selection.len(),
                rename_as.len(),
            ))?;
        }

        headers
            .iter()
            .zip(selection.indexed_mask(headers.len()))
            .map(|(h, o)| if let Some(i) = o { &rename_as[i] } else { h })
            .collect()
    };

    wtr.write_byte_record(&renamed_headers)?;

    while rdr.read_byte_record(&mut record)? {
        wtr.write_byte_record(&record)?;
    }

    Ok(wtr.flush()?)
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn test_slugify() {
        assert_eq!(
            slugify("a two (éléphant)1".as_bytes()),
            "a_two_elephant1".to_string()
        );
    }
}
