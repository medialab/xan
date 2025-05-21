use std::borrow::Cow;
use std::cell::RefCell;
use std::ops::RangeInclusive;

use paltoquet::stemmers::{fr::carry_stemmer, s_stemmer};
use paltoquet::tokenizers::{
    split_paragraphs, split_sentences, NgramsIteratorExt, WordToken, WordTokenKind,
    WordTokenizerBuilder,
};
use pariter::IteratorExt;
use regex::Regex;

use crate::collections::{HashMap, HashSet};
use crate::config::{Config, Delimiter};
use crate::moonblade::{GlobalVariables, Program};
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers, JoinIteratorExt};
use crate::CliResult;

fn get_stemmer(name: &str) -> Result<fn(&str) -> Cow<str>, String> {
    Ok(match name {
        "carry" => |n: &str| Cow::Owned(carry_stemmer(n)),
        "s" => s_stemmer,
        _ => return Err(format!("unknown stemmer \"{}\"", name)),
    })
}

fn parse_range(text: &str) -> Result<RangeInclusive<usize>, &str> {
    let split: Vec<&str> = text.split(',').collect();

    let error_msg = "Could not parse --ngram!";

    if split.len() == 1 {
        let n: usize = split[0].parse().map_err(|_| error_msg)?;
        Ok(n..=n)
    } else if split.len() == 2 {
        let s: usize = split[0].parse().map_err(|_| error_msg)?;
        let e: usize = split[1].parse().map_err(|_| error_msg)?;

        Ok(s..=e)
    } else {
        Err(error_msg)
    }
}

#[derive(Clone)]
enum TokenWhitelist {
    WithId(HashMap<String, String>),
    WithoutId(HashSet<String>),
}

static USAGE: &str = "
Tokenize the given text column by splitting it either into words, sentences
or paragraphs.

# tokenize words

Tokenize the given text column by splitting it into word pieces (think
words, numbers, hashtags etc.).

This tokenizer is able to distinguish between the following types of
tokens (that you can filter using --keep and --drop):
    \"word\", \"number\", \"hashtag\", \"mention\", \"emoji\",
    \"punct\", \"url\" and \"email\"

The command will by default emit one row per row in the input file, with
the tokens added in a new \"tokens\" column containing the processed and filtered
tokens joined by a space (or any character given to --sep).

However, when giving a column name to -T, --token-type, the command will
instead emit one row per token with the token in a new \"token\" column, along
with a new column containing the token's type.

This subcommand also exposes many ways to filter and process the resulting
tokens as well as ways to refine a vocabulary iteratively in tandem with
the \"xan vocab\" command.

Finally, if you still need some processing not covered by the command's flags
you can use -F/--flatmap that lets you evaluate an expression over each token in
order to filter, transform or split them:

Filtering tokens out:

    $ xan tokenize words text -F 'token.startswith(\"Dé\") && token'

Splitting tokens:

    $ xan tokenize words text -F 'token.split(\"-\")'

Transforming tokens:

    $ xan tokenize words text -F 'replace(_, /é/, \"e\")'

# tokenize sentences

Tokenize the given text by splitting it into sentences, emitting one row per
sentence with a new \"sentence\" column at the end.

# tokenize paragraphs

Tokenize the given text by splitting it into paragraphs, emitting one row per
paragraph, with a new \"paragraph\" column at the end.

---

Note that the command will always drop the text column from the
output unless you pass --keep-text to the command.

Tips:

You can easily pipe the command into \"xan vocab\" to create a vocabulary:
    $ xan tokenize words text file.csv | xan vocab doc-token > vocab.csv

You can easily keep the tokens in a separate file using the \"tee\" command:
    $ xan tokenize words text file.csv | tee tokens.csv | xan vocab doc-token > vocab.csv

Usage:
    xan tokenize words [options] <column> [<input>]
    xan tokenize sentences [options] <column> [<input>]
    xan tokenize paragraphs [options] <column> [<input>]
    xan tokenize --help

tokenize options:
    -c, --column <name>      Name for the token column. Will default to \"tokens\", \"token\"
                             when -T/--token-type is provided, \"paragraphs\" or \"sentences\".
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.
    --keep-text              Force keeping the text column in the output.

tokenize words options:
    -S, --simple             Use a simpler, more performant variant of the tokenizer but unable
                             to infer token types, nor handle subtle cases.
    -N, --ngrams <n>         If given, will output token ngrams using the given n or the given
                             range of n values using a comma as separator e.g. \"1,3\".
                             This cannot be used with -T, --token-type.
    -T, --token-type <name>  Name of a column to add containing the type of the tokens.
                             This cannot be used with -N, --ngrams.
    -D, --drop <types>       Types of tokens to drop from the results, separated by comma,
                             e.g. \"word,number\". Cannot work with -k, --keep.
                             See the list of recognized types above.
    -K, --keep <types>       Types of tokens to keep in the results, separated by comma,
                             e.g. \"word,number\". Cannot work with -d, --drop.
                             See the list of recognized types above.
    -m, --min-token <n>      Minimum characters count of a token to be included in the output.
    -M, --max-token <n>      Maximum characters count of a token to be included in the output.
    --stoplist <path>        Path to a .txt stoplist containing one word per line.
    -J, --filter-junk        Whether to apply some heuristics to filter out words that look like junk.
    -L, --lower              Whether to normalize token case using lower case.
    -U, --unidecode          Whether to normalize token text to ascii.
    --split-hyphens          Whether to split tokens by hyphens.
    --stemmer <name>         Stemmer to normalize the tokens. Can be one of:
                                - \"s\": a basic stemmer removing typical plural inflections in
                                         most European languages.
                                - \"carry\": a stemmer targeting the French language.
    -V, --vocab <name>       Path to a CSV file containing allowed vocabulary (or \"-\" for stdin).
    --vocab-token <col>      Column of vocabulary file containing allowed tokens.
                             [default: token]
    --vocab-token-id <col>   Column of vocabulary file containing a token id to emit in place of the
                             token itself.
    --sep <delim>            Character used to join tokens in the output cells. Will default
                             to a space.
    --ngrams-sep <delim>     Separator to be use to join ngrams tokens.
                             [default: §]
    -u, --uniq               Sort and deduplicate the tokens.
    -F, --flatmap <expr>     Evaluate an expression for each extracted token and return nothing,
                             or a transformed token or a list of tokens. The evaluated expression
                             will understand the \"token\" identifier as the currently processed
                             token and \"token_type\" as its type. The expression will run
                             after any of the command's preprocessing toggled through flags,
                             but before deduplication.

tokenize paragraphs options:
    -A, --aerated  Force paragraphs to be separated by a blank line, instead
                   of just a single line break.

tokenize sentences options:
    --squeeze  Collapse consecutive whitespace to produce a tidy output.

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
    cmd_words: bool,
    cmd_sentences: bool,
    cmd_paragraphs: bool,
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
    flag_min_token: Option<usize>,
    flag_max_token: Option<usize>,
    flag_stoplist: Option<String>,
    flag_filter_junk: bool,
    flag_lower: bool,
    flag_unidecode: bool,
    flag_split_hyphens: bool,
    flag_simple: bool,
    flag_ngrams: Option<String>,
    flag_ngrams_sep: String,
    flag_stemmer: Option<String>,
    flag_vocab: Option<String>,
    flag_vocab_token: SelectColumns,
    flag_vocab_token_id: Option<SelectColumns>,
    flag_uniq: bool,
    flag_flatmap: Option<String>,
    flag_aerated: bool,
    flag_squeeze: bool,
}

impl Args {
    fn sep(&self) -> String {
        self.flag_sep.clone().unwrap_or_else(|| " ".to_string())
    }

    fn validate(&self) -> Result<(), &str> {
        if self.cmd_sentences || self.cmd_paragraphs {
            if self.flag_ngrams.is_some() {
                return Err("--ngrams cannot work with paragraphs nor sentences!");
            }

            if self.flag_token_type.is_some() {
                return Err("-T,--token-type cannot work with paragraphs nor sentences!");
            }
        }

        if self.flag_ngrams.is_some() && self.flag_token_type.is_some() {
            return Err("--ngrams cannot be used with -T,--token-type!");
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    args.validate()?;

    let squeeze_regex = Regex::new(r"\s+").unwrap();

    let sep = args.sep();

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_column.clone());

    let ngrams = args
        .flag_ngrams
        .as_ref()
        .map(|text| parse_range(text))
        .transpose()?;

    let stemmer_opt = args
        .flag_stemmer
        .as_ref()
        .map(|name| get_stemmer(name))
        .transpose()?;

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let mut headers = rdr.byte_headers()?.clone();
    let col_index = rconfig.single_selection(&headers)?;

    thread_local! {
        static GLOBALS: RefCell<GlobalVariables> =  {
            let mut globals = GlobalVariables::new();
            globals.register("token");
            globals.register("token_type");
            RefCell::new(globals)
        };
    }

    let flatmap_program_opt = args
        .flag_flatmap
        .as_ref()
        .map(|expr| {
            GLOBALS.with_borrow(|globals| {
                Program::parse_with_globals(&format!("token | {}", expr), &headers, globals)
            })
        })
        .transpose()?;

    let token_column_name = match &args.flag_column {
        Some(name) => name,
        None => {
            if args.cmd_words && args.flag_token_type.is_some() {
                "token"
            } else if args.cmd_paragraphs {
                "paragraph"
            } else if args.cmd_sentences {
                "sentence"
            } else {
                "tokens"
            }
        }
    };

    if !args.flag_no_headers {
        if !args.flag_keep_text {
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
                .map(|name| name.trim_end_matches('s').parse())
                .collect::<Result<Vec<WordTokenKind>, _>>()?,
        );
    } else if let Some(kinds) = args.flag_keep {
        tokenizer_builder = tokenizer_builder.token_kind_whitelist(
            kinds
                .split(',')
                .map(|name| name.trim_end_matches('s').parse())
                .collect::<Result<Vec<WordTokenKind>, _>>()?,
        );
    }

    if let Some(min) = args.flag_min_token {
        tokenizer_builder = tokenizer_builder.min_token_char_count(min);
    }

    if let Some(max) = args.flag_max_token {
        tokenizer_builder = tokenizer_builder.max_token_char_count(max);
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

    let whitelist_opt = args
        .flag_vocab
        .map(|path| -> CliResult<TokenWhitelist> {
            let config = Config::new(&Some(path)).select(args.flag_vocab_token);
            let mut vocab_reader = config.reader()?;
            let vocab_headers = vocab_reader.byte_headers()?;

            let token_pos = config.single_selection(vocab_headers)?;

            let mut vocab_record = csv::ByteRecord::new();

            if let Some(vocab_token_id) = args.flag_vocab_token_id {
                let mut whitelist = HashMap::new();
                let token_id_pos =
                    vocab_token_id.single_selection(vocab_headers, !args.flag_no_headers)?;

                while vocab_reader.read_byte_record(&mut vocab_record)? {
                    let token = String::from_utf8(vocab_record[token_pos].to_vec()).unwrap();
                    let token_id = String::from_utf8(vocab_record[token_id_pos].to_vec()).unwrap();
                    whitelist.insert(token, token_id);
                }

                Ok(TokenWhitelist::WithId(whitelist))
            } else {
                let mut whitelist = HashSet::new();

                while vocab_reader.read_byte_record(&mut vocab_record)? {
                    let token = String::from_utf8(vocab_record[token_pos].to_vec()).unwrap();
                    whitelist.insert(token);
                }

                Ok(TokenWhitelist::WithoutId(whitelist))
            }
        })
        .transpose()?;

    if args.flag_filter_junk {
        tokenizer_builder = tokenizer_builder.filter_junk();
    }

    let tokenizer = tokenizer_builder.build();

    let hyphen_splitter = Regex::new(r"-+").unwrap();

    // NOTE: everything in this function will be parallelized
    let tokenize = move |index: usize,
                         record: &csv::ByteRecord,
                         string: &str|
          -> CliResult<Vec<(String, WordTokenKind)>> {
        if args.cmd_paragraphs {
            return Ok(split_paragraphs(string, args.flag_aerated)
                .map(|paragraph| (paragraph.to_string(), WordTokenKind::Word))
                .collect());
        } else if args.cmd_sentences {
            return Ok(if args.flag_squeeze {
                split_sentences(&squeeze_regex.replace_all(string, " "))
                    .map(|sentence| (sentence.to_string(), WordTokenKind::Word))
                    .collect()
            } else {
                split_sentences(string)
                    .map(|sentence| (sentence.to_string(), WordTokenKind::Word))
                    .collect()
            });
        }

        let string = if args.flag_split_hyphens {
            hyphen_splitter.replace_all(string, " ")
        } else {
            Cow::Borrowed(string)
        };

        let tokens: Box<dyn Iterator<Item = WordToken>> = if args.flag_simple {
            Box::new(tokenizer.simple_tokenize(&string))
        } else {
            Box::new(tokenizer.tokenize(&string))
        };

        let tokens = tokens.filter_map(|token| {
            let pair = token.to_pair();

            let mut text = pair.0;

            if args.flag_lower {
                text = text.to_lowercase();
            }

            if args.flag_unidecode {
                text = unidecode::unidecode(&text);
            }

            if let Some(stemmer) = &stemmer_opt {
                text = stemmer(&text).into_owned();
            }

            if let Some(whitelist) = &whitelist_opt {
                match whitelist {
                    TokenWhitelist::WithoutId(inner) => {
                        if !inner.contains(&text) {
                            return None;
                        }
                    }
                    TokenWhitelist::WithId(inner) => match inner.get(&text) {
                        None => return None,
                        Some(token_id) => {
                            text = token_id.to_string();
                        }
                    },
                }
            }

            Some((text, pair.1))
        });

        let mut collected_tokens: Vec<(String, WordTokenKind)> = if let Some(range) = &ngrams {
            tokens
                .map(|token| token.0)
                .ngrams_range(range.clone())
                .map(|gram| (gram.join(&args.flag_ngrams_sep), WordTokenKind::Word))
                .collect()
        } else {
            tokens.collect()
        };

        if let Some(program) = &flatmap_program_opt {
            let mut flatmapped_tokens = Vec::with_capacity(collected_tokens.len());

            for (token, kind) in collected_tokens.into_iter() {
                GLOBALS.with_borrow_mut(|globals| -> CliResult<()> {
                    globals.set(0, token);

                    // TODO: we could avoid setting the type when we know it is not used
                    globals.set(1, kind.as_str());

                    let result = program.run_with_record_and_globals(index, record, globals)?;

                    for value in result.flat_iter() {
                        if value.is_falsey() {
                            continue;
                        }

                        flatmapped_tokens.push((value.try_as_str()?.into_owned(), kind));
                    }

                    Ok(())
                })?;
            }

            collected_tokens = flatmapped_tokens;
        }

        if args.flag_uniq {
            collected_tokens.sort_by(|a, b| a.0.cmp(&b.0));
            collected_tokens.dedup_by(|a, b| a.0 == b.0);
        }

        Ok(collected_tokens)
    };

    // NOTE: nothing here will be parallelized
    macro_rules! write_tokens {
        ($record:ident, $tokens:expr) => {{
            if args.cmd_paragraphs || args.cmd_sentences {
                for token in $tokens {
                    let mut record_to_write = if args.flag_keep_text {
                        $record.clone()
                    } else {
                        $record.remove(col_index)
                    };

                    record_to_write.push_field(token.0.as_bytes());

                    wtr.write_record(&record_to_write)?;
                }
            } else if args.flag_token_type.is_some() {
                for token in $tokens {
                    let mut record_to_write = if args.flag_keep_text {
                        $record.clone()
                    } else {
                        $record.remove(col_index)
                    };

                    record_to_write.push_field(token.0.as_bytes());
                    record_to_write.push_field(token.1.as_str().as_bytes());

                    wtr.write_record(&record_to_write)?;
                }
            } else {
                let mut record_to_write = if args.flag_keep_text {
                    $record.clone()
                } else {
                    $record.remove(col_index)
                };

                let joined_tokens = $tokens.iter().map(|token| token.0.as_str()).join(&sep);

                record_to_write.push_field(joined_tokens.as_bytes());

                wtr.write_byte_record(&record_to_write)?;
            }
        }};
    }

    if let Some(threads) = parallelization {
        rdr.into_byte_records()
            .enumerate()
            .parallel_map_custom(
                |o| {
                   o.threads(threads.unwrap_or_else(num_cpus::get))
                },
                move |(index, result)| -> CliResult<(csv::ByteRecord, Vec<(String, WordTokenKind)>)> {
                    let record = result?;

                    let text =
                        std::str::from_utf8(&record[col_index]).expect("could not decode utf8");

                    tokenize(index, &record, text).map(|tokens| {
                        (record, tokens)
                    })
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (record, tokens) = result?;

                write_tokens!(record, tokens);

                Ok(())
            })?;
    } else {
        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while rdr.read_byte_record(&mut record)? {
            let text = std::str::from_utf8(&record[col_index]).expect("could not decode utf8");
            let tokens = tokenize(index, &record, text)?;

            write_tokens!(record, tokens);

            index += 1;
        }
    }

    Ok(wtr.flush()?)
}
