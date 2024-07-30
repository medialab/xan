use paltoquet::{WordToken, WordTokenKind, WordTokenizerBuilder};
use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

// TODO: all kind of min/max len, stoplist, --separator, --keep-text

static USAGE: &str = "
Tokenize the given text column and emit one row per token in a new column
at the end, all while dropping the original text column.

This tokenizer is able to distinguish between the following types of tokens:
    - word
    - number
    - hashtag
    - mention
    - emoji
    - punct
    - url
    - email

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
    -D, --drop <kinds>       Kinds of tokens to drop from the results, separated by comma,
                             e.g. \"word,number\". Cannot work with -k, --keep.
    -K, --keep <kinds>       Kinds of tokens to keep in the results, separated by comma,
                             e.g. \"word,number\". Cannot work with -d, --drop.
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
    flag_drop: Option<String>,
    flag_keep: Option<String>,
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
        let mut renamed_headers = headers.remove(col_index);
        renamed_headers.push_field(args.flag_column.as_bytes());

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

    let mut tokenizer_builder = WordTokenizerBuilder::new();

    if let Some(kinds) = args.flag_drop {
        tokenizer_builder = tokenizer_builder.token_kind_blacklist(
            kinds
                .split(',')
                .into_iter()
                .map(|name| name.parse())
                .collect::<Result<Vec<WordTokenKind>, _>>()?,
        );
    } else if let Some(kinds) = args.flag_keep {
        tokenizer_builder = tokenizer_builder.token_kind_whitelist(
            kinds
                .split(',')
                .into_iter()
                .map(|name| name.parse())
                .collect::<Result<Vec<WordTokenKind>, _>>()?,
        );
    }

    let tokenizer = tokenizer_builder.build();

    macro_rules! write_tokens {
        ($record:ident, $tokens:expr) => {
            for token in $tokens {
                let mut record_to_write = $record.remove(col_index);
                record_to_write.push_field(token.text.as_bytes());

                if args.flag_token_type.is_some() {
                    record_to_write.push_field(token.kind.as_str().as_bytes());
                }

                wtr.write_record(&record_to_write)?;
            }
        };
    }

    if let Some(threads) = parallelization {
        rdr.into_byte_records()
            .parallel_map_custom(
                |o| {
                    if let Some(count) = threads {
                        o.threads(count)
                    } else {
                        o
                    }
                },
                move |result| -> CliResult<(csv::ByteRecord, Vec<(String, WordTokenKind)>)> {
                    let record = result?;

                    let text =
                        std::str::from_utf8(&record[col_index]).expect("could not decode utf8");

                    let tokens = tokenizer
                        .tokenize(text)
                        .map(|token| (token.text.to_string(), token.kind))
                        .collect();

                    Ok((record, tokens))
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (record, tokens) = result?;

                write_tokens!(
                    record,
                    tokens
                        .iter()
                        .map(|(text, kind)| WordToken { text, kind: *kind })
                );

                Ok(())
            })?;
    } else {
        let mut record = csv::ByteRecord::new();

        while rdr.read_byte_record(&mut record)? {
            let text = std::str::from_utf8(&record[col_index]).expect("could not decode utf8");

            write_tokens!(record, tokenizer.tokenize(text))
        }
    }

    Ok(wtr.flush()?)
}
