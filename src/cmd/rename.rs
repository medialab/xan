use csv;

use config::{Config, Delimiter};
use select::{SelectColumns, Selection};
use util;
use CliResult;

static USAGE: &str = "
Rename columns of a CSV file. Can also be used to add headers to a headless
CSV file.

Renaming all columns:

    $ xsv rename NAME,SURNAME file.csv

Renaming a selection of columns:

    $ xsv rename NAME -s name file.csv

Adding a header to a headless file:

    $ xsv rename -n name,surname file.csv

Prefixing column names:

    $ xsv rename --prefix university_ file.csv

The renamed column must be passed in CSV format:

    $ xsv rename '\"NAME OF PERSON\",AGE' file.csv

Usage:
    xsv rename [options] --prefix <prefix> [<input>]
    xsv rename [options] <columns> [<input>]
    xsv rename --help

rename options:
    -s, --select <arg>     Select the columns to search. See 'xsv select -h'
                           for the full syntax.
    -p, --prefix <prefix>  Prefix to add to all the column names.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. [default: ,]
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_columns: Option<String>,
    flag_select: Option<SelectColumns>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_prefix: Option<String>,
}

// TODO: no headers

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let mut rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = rconfig.reader()?;
    let headers = rdr.byte_headers()?;

    let selection = match args.flag_select {
        Some(selection) => {
            rconfig = rconfig.select(selection);
            rconfig.selection(headers)?
        }
        None => Selection::full(headers.len()),
    };

    let renamed_headers: csv::ByteRecord = if let Some(prefix) = args.flag_prefix {
        headers
            .iter()
            .zip(selection.indexed_mask(headers.len()).into_iter())
            .map(|(h, o)| {
                if o.is_some() {
                    [prefix.as_bytes(), h].concat()
                } else {
                    h.to_vec()
                }
            })
            .collect()
    } else {
        let rename_as = util::str_to_csv_byte_record(&args.arg_columns.unwrap());

        if selection.len() != rename_as.len() {
            return fail!(format!(
                "Renamed columns alignement error. Expected {} names and got {}.",
                selection.len(),
                rename_as.len(),
            ));
        }

        headers
            .iter()
            .zip(selection.indexed_mask(headers.len()).into_iter())
            .map(|(h, o)| if let Some(i) = o { &rename_as[i] } else { h })
            .collect()
    };

    let mut wtr = Config::new(&args.flag_output).writer()?;
    wtr.write_byte_record(&renamed_headers)?;

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        wtr.write_byte_record(&record)?;
    }

    Ok(wtr.flush()?)
}
