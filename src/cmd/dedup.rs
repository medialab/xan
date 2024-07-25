use std::collections::HashSet;

use csv;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Deduplicate the rows of a CSV file. Runs in O(n) time, consuming O(c) memory, c being
the distinct number of row identities.

If your file is already sorted on the deduplication selection, use the -S/--sorted flag
to run in O(1) memory instead.

Note that it will be the first row having a specific identity that will be emitted in
the output and not any subsequent one.

Usage:
    xan dedup [options] [<input>]
    xan dedup --help

dedup options:
    -s, --select <arg>  Select a subset of columns to on which to deduplicate.
                        See 'xan select --help' for the format details.
    -S, --sorted        Use if you know your file is already sorted on the deduplication
                        selection to avoid storing unique values in memory.
    -l, --keep-last     Keep the last row having a specific identiy, rather than
                        the first one.

Common options:
    -h, --help               Display this message
    -o, --output <file>      Write output to <file> instead of stdout.
    -n, --no-headers         When set, the first row will not be evaled
                             as headers.
    -d, --delimiter <arg>    The field delimiter for reading CSV data.
                             Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_no_headers: bool,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_sorted: bool,
    flag_keep_last: bool,
}

type DeduplicationKey = Vec<Vec<u8>>;

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconf.reader()?;
    let sel = rconf.selection(rdr.byte_headers()?)?;

    let mut wtr = Config::new(&args.flag_output).writer()?;

    rconf.write_headers(&mut rdr, &mut wtr)?;

    if !args.flag_sorted {
        let mut record = csv::ByteRecord::new();
        let mut already_seen = HashSet::<DeduplicationKey>::new();

        while rdr.read_byte_record(&mut record)? {
            let key = sel.collect(&record);

            if already_seen.insert(key) {
                wtr.write_byte_record(&record)?;
            }
        }
    } else if args.flag_keep_last {
        let mut current: Option<(DeduplicationKey, csv::ByteRecord)> = None;

        for result in rdr.byte_records() {
            let record = result?;
            let key = sel.collect(&record);

            match current {
                Some((current_key, record_to_flush)) if current_key != key => {
                    wtr.write_byte_record(&record_to_flush)?;
                }
                _ => (),
            }

            current = Some((key, record));
        }

        if let Some((_, record_to_flush)) = current {
            wtr.write_byte_record(&record_to_flush)?;
        }
    } else {
        let mut record = csv::ByteRecord::new();
        let mut current: Option<DeduplicationKey> = None;

        while rdr.read_byte_record(&mut record)? {
            let key = sel.collect(&record);

            match current {
                None => {
                    wtr.write_byte_record(&record)?;
                    current = Some(key);
                }
                Some(current_key) if current_key != key => {
                    wtr.write_byte_record(&record)?;
                    current = Some(key);
                }
                _ => (),
            };
        }
    }

    Ok(wtr.flush()?)
}
