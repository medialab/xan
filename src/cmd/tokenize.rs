use paltoquet::{WordToken, WordTokenKind, WordTokenizerBuilder};
use pariter::IteratorExt;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

// TODO: simple tokenizer

static USAGE: &str = "
Tokenize the given text column by splitting it into word pieces (think
words, numbers, hashtags etc.). This command will therefore emit one row
per token written in a new column added at the end, all while dropping
the original text column unless --sep or --keep-text is passed.

For instance, given the following input:

id,text
1,one cat eats 2 mice! 😎
2,hello

The following command:
    $ xan tokenize text -T type file.csv

Will produce the following result:

id,token,type
1,one,word
1,cat,word
1,eats,word
1,2,number
1,mice,word
1,!,punct
1,😎,emoji
2,hello,word

You can easily pipe the command into \"xan frequency\" to create a vocabulary:
    $ xan tokenize text file.csv | xan frequency -s token -l 0 > vocab.csv

You can easily keep the tokens in a separate file using the \"tee\" command:
    $ xan tokenize text file.csv | tee tokens.csv | xan frequency -s token -l 0 > vocab.csv

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
    -c, --column <name>      Name for the token column. Will default to \"token\" or \"tokens\"
                             if --sep is given.
    -T, --token-type <name>  Name of a column to add containing the type of the tokens.
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -D, --drop <types>       Types of tokens to drop from the results, separated by comma,
                             e.g. \"word,number\". Cannot work with -k, --keep.
                             See the list of recognized types above.
    -K, --keep <types>       Types of tokens to keep in the results, separated by comma,
                             e.g. \"word,number\". Cannot work with -d, --drop.
                             See the list of recognized types above.
    -m, --min-token-len <n>  Minimum length of a token to be included in the output.
    -M, --max-token-len <n>  Maximum length of a token to be included in the output.
    --stoplist <path>        Path to a .txt stoplist containing one word per line.
    --sep <char>             If given, the command will output exactly one row per input row,
                             keep the text column and join the tokens using the provided character.
                             We recommend using \"§\" as a separator.
    --keep-text              Force keeping the text column in output.
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
    flag_column: Option<String>,
    flag_token_type: Option<String>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
    flag_drop: Option<String>,
    flag_keep: Option<String>,
    flag_sep: Option<String>,
    flag_keep_text: bool,
    flag_min_token_len: Option<usize>,
    flag_max_token_len: Option<usize>,
    flag_stoplist: Option<String>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column.clone());

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let col_index = rconfig.single_selection(&headers)?;

    let token_column_name = match &args.flag_column {
        Some(name) => name,
        None => {
            if args.flag_sep.is_some() {
                "tokens"
            } else {
                "token"
            }
        }
    };

    if !args.flag_no_headers {
        if !args.flag_sep.is_some() && !args.flag_keep_text {
            headers = headers.remove(col_index);
        }
        headers.push_field(token_column_name.as_bytes());

        if let Some(name) = &args.flag_token_type {
            headers.push_field(name.as_bytes());
        }

        wtr.write_byte_record(&headers)?;
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
                .map(|name| name.parse())
                .collect::<Result<Vec<WordTokenKind>, _>>()?,
        );
    } else if let Some(kinds) = args.flag_keep {
        tokenizer_builder = tokenizer_builder.token_kind_whitelist(
            kinds
                .split(',')
                .map(|name| name.parse())
                .collect::<Result<Vec<WordTokenKind>, _>>()?,
        );
    }

    if let Some(min) = args.flag_min_token_len {
        tokenizer_builder = tokenizer_builder.min_token_len(min);
    }

    if let Some(max) = args.flag_max_token_len {
        tokenizer_builder = tokenizer_builder.max_token_len(max);
    }

    if let Some(path) = args.flag_stoplist {
        let mut contents = String::new();

        Config::new(&Some(path))
            .io_reader()?
            .read_to_string(&mut contents)?;

        for word in contents.lines() {
            tokenizer_builder.insert_stopword(word);
        }
    }

    let tokenizer = tokenizer_builder.build();

    macro_rules! write_tokens {
        ($record:ident, $tokens:expr) => {{
            if let Some(sep) = &args.flag_sep {
                let mut types = Vec::new();

                let joined_tokens = $tokens
                    .map(|t| {
                        if args.flag_token_type.is_some() {
                            types.push(t.kind.as_str());
                        }
                        t.text
                    })
                    .collect::<Vec<_>>()
                    .join(sep);

                // NOTE: if not -p, we are mutating the working record
                $record.push_field(joined_tokens.as_bytes());

                if args.flag_token_type.is_some() {
                    $record.push_field(types.join(sep).as_bytes());
                }

                wtr.write_byte_record(&$record)?;
            } else {
                for token in $tokens {
                    let mut record_to_write = if args.flag_keep_text {
                        $record.clone()
                    } else {
                        $record.remove(col_index)
                    };

                    record_to_write.push_field(token.text.as_bytes());

                    if args.flag_token_type.is_some() {
                        record_to_write.push_field(token.kind.as_str().as_bytes());
                    }

                    wtr.write_record(&record_to_write)?;
                }
            }
        }};
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
                let (mut record, tokens) = result?;

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
