use paltoquet::{Tokenize, WordTokenKind};
use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

// TODO: all kind of filters, --separator

static USAGE: &str = "
Tokenize the given text column and emit one row per token, replacing
the text column by a token one.

Usage:
    xan tokenize [options] <column> [<input>]
    xan tokenize --help

tokenize options:
    -c, --column <name>      Name for the token column [default: token].
    -T, --token-type <name>  Name for the token type column to append.
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

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
    arg_column: SelectColumns,
    arg_input: Option<String>,
    flag_column: String,
    flag_token_type: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let col_index = rconfig.single_selection(&headers)?;

    if !args.flag_no_headers {
        let mut renamed_headers = headers.replace_at(col_index, args.flag_column.as_bytes());

        if let Some(name) = &args.flag_token_type {
            renamed_headers.push_field(name.as_bytes());
        }

        wtr.write_byte_record(&renamed_headers)?;
    }

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    if let Some(threads) = parallelization {
        rdr.into_records()
            .parallel_map_custom(
                |o| {
                    if let Some(count) = threads {
                        o.threads(count)
                    } else {
                        o
                    }
                },
                move |result| -> CliResult<(csv::StringRecord, Vec<(String, WordTokenKind)>)> {
                    let record = result?;

                    let tokens = record[col_index]
                        .words()
                        .map(|token| (token.text.to_string(), token.kind))
                        .collect();

                    Ok((record, tokens))
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (record, tokens) = result?;

                for (text, kind) in tokens {
                    let mut record_to_write = record.replace_at(col_index, &text);

                    if args.flag_token_type.is_some() {
                        record_to_write.push_field(kind.as_str());
                    }

                    wtr.write_record(&record_to_write)?;
                }

                Ok(())
            })?;
    } else {
        let mut record = csv::StringRecord::new();

        while rdr.read_record(&mut record)? {
            let text = &record[col_index];

            for token in text.words() {
                let mut record_to_write = record.replace_at(col_index, token.text);

                if args.flag_token_type.is_some() {
                    record_to_write.push_field(token.kind.as_str());
                }

                wtr.write_record(&record_to_write)?;
            }
        }
    }

    Ok(wtr.flush()?)
}
