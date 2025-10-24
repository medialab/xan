use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Fill empty cells of a CSV file by filling them with any non-empty value seen
before (this is usually called forward filling), or with any constant value
given to the -v, --value flag.

For instance, replacing empty values with 0 everywhere in the file:

    $ xan fill -v 0 data.csv > filled.csv

Usage:
    xan fill [options] [<input>]
    xan fill --help

fill options:
    -s, --select <cols>  Selection of columns to fill.
    -v, --value <value>  Fill empty cells using provided value instead of using
                         last non-empty value.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the file will be considered as having no
                           headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_value: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconf.simd_reader()?;
    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    let headers = rdr.byte_headers()?;

    let sel = rconf.selection(headers)?;
    let mask = sel.mask(headers.len());

    if !args.flag_no_headers {
        wtr.write_byte_record(headers)?;
    }

    let mut previous: Option<simd_csv::ByteRecord> = None;

    for result in rdr.byte_records() {
        let record = result?;

        // Default value
        if let Some(value) = &args.flag_value {
            wtr.write_record(mask.iter().copied().enumerate().map(|(i, should_fill)| {
                let current_cell = &record[i];

                match (should_fill, current_cell.is_empty()) {
                    (true, true) => value.as_bytes(),
                    _ => current_cell,
                }
            }))?;
        }
        // Forward filling
        else {
            match previous.as_mut() {
                None => {
                    wtr.write_byte_record(&record)?;
                    previous = Some(record);
                }
                Some(previous_record) => {
                    let filled_record = mask
                        .iter()
                        .enumerate()
                        .map(|(i, should_fill)| {
                            let current_cell = &record[i];

                            match (should_fill, current_cell.is_empty()) {
                                (true, true) => &previous_record[i],
                                _ => current_cell,
                            }
                        })
                        .collect::<simd_csv::ByteRecord>();

                    wtr.write_byte_record(&filled_record)?;

                    *previous_record = filled_record;
                }
            };
        }
    }

    Ok(wtr.flush()?)
}
