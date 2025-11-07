// References:
// Cointet, Jean-Philippe. La cartographie des traces textuelles comme méthodologie d’enquête en
// sciences sociales. 2017. École normale supérieure, thesis. hal-bioemco.ccsd.cnrs.fr,
// https://sciencespo.hal.science/tel-03626011
// https://sciencespo.hal.science/tel-03626011v1/file/2017-cointet-hdr-la-cartographie-des-traces-textuelles-comme-methodologie-denquete-en-sciences-sociales.pdf
// https://pbil.univ-lyon1.fr/R/pdf/tdr35.pdf

use std::cmp::Ordering;
use std::convert::TryFrom;
use std::num::NonZeroUsize;
use std::rc::Rc;

use bstr::ByteSlice;
use simd_csv::ByteRecord;

use crate::collections::ClusteredInsertHashmap;
use crate::collections::{hash_map::Entry, HashMap};
use crate::config::{Config, Delimiter};
use crate::select::SelectedColumns;
use crate::util;
use crate::CliError;
use crate::CliResult;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(try_from = "String")]
struct SignificanceLevel(f64);

impl SignificanceLevel {
    fn get(&self) -> f64 {
        self.0
    }
}

impl TryFrom<String> for SignificanceLevel {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        // Thresholds for k=1
        Ok(Self(match value.as_str() {
            ".5" | "0.5" => 0.45,
            ".1" | "0.1" => 2.71,
            ".05" | "0.05" => 3.84,
            ".025" | "0.025" => 5.02,
            ".01" | "0.01" => 6.63,
            ".005" | "0.005" => 7.88,
            ".001" | "0.001" => 10.83,
            _ => return Err(format!("unsupported significance threshold \"{}\"", &value)),
        }))
    }
}

static USAGE: &str = "
Compute vocabulary statistics over tokenized documents (typically produced
by the \"xan tokenize words\" subcommand), i.e. rows of CSV data containing
a \"tokens\" column containing word tokens separated by a single space (or
any separator given to the --sep flag).

The command considers, by default, documents to be a single row of the input
but can also be symbolized by the value of a column selection given to -D/--doc.

This command can compute 5 kinds of differents vocabulary statistics:

1. corpus-level statistics (using the \"corpus\" subcommand):
    - doc_count: number of documents in the corpus
    - token_count: total number of tokens in the corpus
    - distinct_token_count: number of distinct tokens in the corpus
    - average_doc_len: average number of tokens per document

2. token-level statistics (using the \"token\" subcommand):
    - token: some distinct token (the column will be named like the input)
    - gf: global frequency of the token across corpus
    - df: document frequency of the token
    - df_ratio: proportion of documents containing the token
    - idf: logarithm of the inverse document frequency of the token
    - gfidf: global frequency * idf for the token
    - pigeon: ratio between df and expected df in random distribution

3. doc-level statistics (using the \"doc\" subcommand):
    - (*doc): columns representing the document (named like the input)
    - token_count: total number of tokens in document
    - distinct_token_count: number of distinct tokens in document

4. doc-token-level statistics (using the \"doc-token\" subcommand):
    - (*doc): columns representing the document (named like the input)
    - token: some distinct document token (the column will be named like the input)
    - tf: term frequency for the token in the document
    - expected_tf: expected absolute term frequency (does not follow --tf-weight)
    - tfidf: term frequency * idf for the token in the document
    - bm25: BM25 score for the token in the document
    - chi2: chi2 score for the token in the document

5. token-cooccurrence-level statistics (using the \"cooc\" subcommand):
    - token1: the first token
    - token2: the second token
    - count: number of co-occurrences
    - expected_count: expected number of co-occurrences
    - chi2: chi2 score (approx. without the --complete flag)
    - G2: G2 score (approx. without the --complete flag)
    - pmi: pointwise mutual information
    - npmi: normalized pointwise mutual information

    or, using the --distrib flag:

    - token1: the first token
    - token2: the second token
    - count: number of co-occurrences
    - expected_count: expected number of co-occurrences
    - sd_I: distributional score based on PMI
    - sd_G2: distributional score based on G2

    or, using the --specificity flag (NOT CORRECT YET! DO NOT USE!):

    - token: the token
    - count: total number of co-occurrences
    - lgl: the specificity score (ratio of statistically relevant co-occurrences)

Note that you should generally avoid giving too much importance wrt
the statistical relevance of both chi2 & G2 scores when considering
less than 5 items (absolute term frequencies or co-occurrence counts).

Usage:
    xan vocab corpus [options] [<input>]
    xan vocab token [options] [<input>]
    xan vocab doc [options] [<input>]
    xan vocab doc-token [options] [<input>]
    xan vocab cooc [options] [<input>]
    xan vocab --help

vocab options:
    -T, --token <token-col>  Name of column containing the tokens. Will default
                             to \"tokens\" or \"token\" if --implode is given.
    -D, --doc <doc-cols>     Optional selection of columns representing a row's document.
                             Each row of input will be considered as its own document if
                             the flag is not given.
    --sep <delim>            Delimiter used to separate tokens in one row's token cell.
                             Will default to a single space.
    --implode                If given, will implode the file over the token column so that
                             it becomes possible to process a file containing only one token
                             per row. Cannot be used without -D, --doc.

vocab doc-token options:
    --tf-weight <weight>         TF weighting scheme. One of \"count\", \"binary\", \"ratio\",
                                 or \"log-normal\". [default: count]
    --k1-value <value>           \"k1\" Factor for BM25 computation. [default: 1.2]
    --b-value <value>            \"b\"  Factor for BM25 computation. [default: 0.75]
    --chi2-significance <value>  Filter doc,token pairs by only keeping significant ones wrt their
                                 chi2 score that must be above the given significance level. Accepted
                                 levels include \"0.5\", \"0.1\", \"0.05\", \"0.025\", \"0.01\",
                                 \"0.005\" and \"0.001\".

vocab cooc options:
    -w, --window <n>             Size of the co-occurrence window, in number of tokens around the currently
                                 considered token. If not given, co-occurrences will be computed using the bag
                                 of words model where tokens are considered to co-occur with every
                                 other one in the same document.
                                 Set the window to \"1\" to compute bigram collocations. Set a larger window
                                 to get something similar to what word2vec would consider.
    -F, --forward                Whether to only consider a forward window when traversing token contexts.
    --distrib                    Compute directed distributional similarity metrics instead.
    --specificity                Compute the lgl specificity score per token instead.
    --min-count <n>              Minimum number of co-occurrence count to be included in the result.
                                 [default: 1]
    --chi2-significance <value>  Filter doc,token pairs by only keeping significant ones wrt their
                                 chi2 score that must be above the given significance level. Accepted
                                 levels include \"0.5\", \"0.1\", \"0.05\", \"0.025\", \"0.01\",
                                 \"0.005\" and \"0.001\".
    --G2-significance <value>    Filter doc,token pairs by only keeping significant ones wrt their
                                 G2 score that must be above the given significance level. Accepted
                                 levels include \"0.5\", \"0.1\", \"0.05\", \"0.025\", \"0.01\",
                                 \"0.005\" and \"0.001\".

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Clone, Deserialize)]
struct Args {
    cmd_token: bool,
    cmd_doc: bool,
    cmd_doc_token: bool,
    cmd_corpus: bool,
    cmd_cooc: bool,
    arg_input: Option<String>,
    flag_token: Option<SelectedColumns>,
    flag_doc: Option<SelectedColumns>,
    flag_sep: Option<String>,
    flag_implode: bool,
    flag_tf_weight: TfWeighting,
    flag_k1_value: f64,
    flag_b_value: f64,
    flag_chi2_significance: Option<SignificanceLevel>,
    #[serde(rename = "flag_G2_significance")]
    flag_g2_significance: Option<SignificanceLevel>,
    flag_window: Option<NonZeroUsize>,
    flag_forward: bool,
    flag_distrib: bool,
    flag_specificity: bool,
    flag_min_count: usize,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_implode {
            self.flag_sep = None;
        } else if self.flag_sep.is_none() {
            self.flag_sep = Some(" ".to_string());
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve();

    if args.flag_doc.is_none() && args.flag_sep.is_none() {
        return Err(CliError::Other(
            "cannot omit -D, --doc without --sep!".to_string(),
        ));
    }

    if (args.flag_distrib || args.flag_specificity) && args.flag_forward {
        return Err(CliError::Other(
            "-D, --distrib does not make sense with -F, --forward".to_string(),
        ));
    }

    let chi2_significance = args.flag_chi2_significance.map(|s| s.get());
    let g2_significance = args.flag_g2_significance.map(|s| s.get());

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers);

    let mut rdr = rconf.simd_reader()?;
    let headers = rdr.byte_headers()?.clone();

    let flag_token = args.flag_token.unwrap_or_else(|| {
        SelectedColumns::parse(if args.flag_implode { "token" } else { "tokens" }).unwrap()
    });

    let token_pos = flag_token.single_selection(&headers, !rconf.no_headers)?;

    let doc_sel = args
        .flag_doc
        .map(|s| s.selection(&headers, !rconf.no_headers))
        .transpose()?;

    let mut record = ByteRecord::new();
    let mut i: usize = 0;

    let mut wtr = Config::new(&args.flag_output).simd_writer()?;

    if args.cmd_cooc {
        let mut cooccurrences = Cooccurrences::default();

        let cooccurrence_mode = if args.flag_forward {
            CooccurrenceMode::Forward
        } else if args.flag_distrib || args.flag_specificity {
            CooccurrenceMode::Full
        } else {
            CooccurrenceMode::Symmetrical
        };

        // NOTE:
        //  --sep and no --doc: trivial for both model
        //  --sep and --doc:
        //      need a multimap for bow
        //  no --sep and --doc:
        //      need a multimap for bow
        //      need to aggregate consecutive identical doc value for window
        match (&args.flag_sep, &doc_sel, args.flag_window) {
            // Separator or not, doc column, bag-of-words model
            (_, Some(sel), None) => {
                let mut doc_tokens: ClusteredInsertHashmap<Document, Vec<Rc<Token>>> =
                    ClusteredInsertHashmap::new();

                while rdr.read_byte_record(&mut record)? {
                    let doc = sel.select(&record).collect();

                    doc_tokens.insert_with_or_else(
                        doc,
                        || match &args.flag_sep {
                            Some(sep) => record[token_pos]
                                .split_str(sep)
                                .map(|t| Rc::new(t.to_vec()))
                                .collect(),
                            None => {
                                vec![Rc::new(record[token_pos].to_vec())]
                            }
                        },
                        |tokens| {
                            match &args.flag_sep {
                                Some(sep) => {
                                    for token in record[token_pos].split_str(sep) {
                                        tokens.push(Rc::new(token.to_vec()));
                                    }
                                }
                                None => {
                                    tokens.push(Rc::new(record[token_pos].to_vec()));
                                }
                            };
                        },
                    );
                }

                for bag_of_words in doc_tokens.into_values() {
                    for i in 0..bag_of_words.len() {
                        let source = &bag_of_words[i];
                        let source_id = cooccurrences.register_token(source.clone());

                        #[allow(clippy::needless_range_loop)]
                        for j in (i + 1)..bag_of_words.len() {
                            let target = &bag_of_words[j];
                            let target_id = cooccurrences.register_token(target.clone());
                            cooccurrences.add_cooccurrence(cooccurrence_mode, source_id, target_id);
                        }
                    }
                }
            }

            // Separator, doc = row or sel, both models
            (Some(sep), _, model) => {
                while rdr.read_byte_record(&mut record)? {
                    let bag_of_words: Vec<Rc<Token>> = record[token_pos]
                        .split_str(sep)
                        .map(|t| Rc::new(t.to_vec()))
                        .collect();

                    for i in 0..bag_of_words.len() {
                        let source = &bag_of_words[i];
                        let source_id = cooccurrences.register_token(source.clone());

                        let upper_bound = model
                            .map(|window| (i + 1 + window.get()).min(bag_of_words.len()))
                            .unwrap_or(bag_of_words.len());

                        #[allow(clippy::needless_range_loop)]
                        for j in (i + 1)..upper_bound {
                            let target = &bag_of_words[j];
                            let target_id = cooccurrences.register_token(target.clone());
                            cooccurrences.add_cooccurrence(cooccurrence_mode, source_id, target_id);
                        }
                    }
                }
            }

            // No separator, doc = sel, window model
            (None, Some(sel), Some(window)) => {
                macro_rules! flush_bag_of_words {
                    ($bag_of_words:ident) => {{
                        for i in 0..$bag_of_words.len() {
                            let source = &$bag_of_words[i];
                            let source_id = cooccurrences.register_token(source.clone());

                            #[allow(clippy::needless_range_loop)]
                            for j in (i + 1)..(i + 1 + window.get()).min($bag_of_words.len()) {
                                let target = &$bag_of_words[j];
                                let target_id = cooccurrences.register_token(target.clone());
                                cooccurrences.add_cooccurrence(
                                    cooccurrence_mode,
                                    source_id,
                                    target_id,
                                );
                            }
                        }
                    }};
                }

                let mut current_opt: Option<(Document, Vec<Rc<Token>>)> = None;

                while rdr.read_byte_record(&mut record)? {
                    let doc = sel.select(&record).collect();
                    let token = Rc::new(record[token_pos].to_vec());

                    match current_opt.as_mut() {
                        Some((current_doc, tokens)) if current_doc == &doc => {
                            tokens.push(token);
                        }
                        _ => {
                            if let Some((_, tokens)) = current_opt.replace((doc, vec![token])) {
                                flush_bag_of_words!(tokens);
                            }
                        }
                    };
                }

                if let Some((_, tokens)) = current_opt {
                    flush_bag_of_words!(tokens);
                }
            }

            // Not possible, as per arg validation (no sep without doc is not possible since irrelevant)
            _ => unreachable!(),
        };

        if args.flag_distrib {
            let output_headers: [&[u8]; 6] = [
                b"token1",
                b"token2",
                b"count",
                b"expected_count",
                b"sd_I",
                b"sd_G2",
            ];

            wtr.write_record(output_headers)?;
            cooccurrences
                .for_each_distrib_cooc_record(args.flag_min_count, |r| wtr.write_byte_record(r))?;
        } else if args.flag_specificity {
            let output_headers: [&[u8]; 3] = [b"token", b"count", b"lgl"];

            wtr.write_record(output_headers)?;
            cooccurrences.for_each_specificity_record(
                args.flag_min_count,
                g2_significance,
                |r| wtr.write_byte_record(r),
            )?;
        } else {
            let output_headers: [&[u8]; 8] = [
                b"token1",
                b"token2",
                b"count",
                b"expected_count",
                b"chi2",
                b"G2",
                b"pmi",
                b"npmi",
            ];

            wtr.write_record(output_headers)?;
            cooccurrences.for_each_cooc_record(
                args.flag_min_count,
                chi2_significance,
                g2_significance,
                |r| wtr.write_byte_record(r),
            )?;
        }

        return Ok(wtr.flush()?);
    }

    let mut vocab = Vocabulary::new();

    while rdr.read_byte_record(&mut record)? {
        let document: Document = match &doc_sel {
            Some(sel) => sel.select(&record).collect(),
            None => {
                let mut doc_key = ByteRecord::new();
                doc_key.push_field(i.to_string().as_bytes());
                doc_key
            }
        };

        if let Some(sep) = &args.flag_sep {
            for token in record[token_pos].split_str(sep) {
                let token: Token = token.trim().to_vec();

                if !token.is_empty() {
                    vocab.add(document.clone(), token);
                }
            }
        } else {
            let token: Token = record[token_pos].trim().to_vec();

            if !token.is_empty() {
                vocab.add(document, token);
            }
        }

        i += 1;
    }

    if args.cmd_token {
        let headers: [&[u8]; 7] = [
            b"token",
            b"gf",
            b"df",
            b"df_ratio",
            b"idf",
            b"gfidf",
            b"pigeon",
        ];
        wtr.write_record(headers)?;
        vocab.for_each_token_level_record(|r| wtr.write_byte_record(r))?;
    } else if args.cmd_doc_token {
        let mut output_headers = ByteRecord::new();

        if let Some(sel) = &doc_sel {
            for col_name in sel.select(&headers) {
                output_headers.push_field(col_name);
            }
        } else {
            output_headers.push_field(b"doc");
        }

        output_headers.push_field(b"token");
        output_headers.push_field(b"tf");
        output_headers.push_field(b"expected_tf");
        output_headers.push_field(b"tfidf");
        output_headers.push_field(b"bm25");
        output_headers.push_field(b"chi2");

        wtr.write_byte_record(&output_headers)?;
        vocab.for_each_doc_token_level_record(
            args.flag_k1_value,
            args.flag_b_value,
            args.flag_tf_weight,
            chi2_significance,
            |r| wtr.write_byte_record(r),
        )?;
    } else if args.cmd_doc {
        let mut output_headers = ByteRecord::new();

        if let Some(sel) = &doc_sel {
            for col_name in sel.select(&headers) {
                output_headers.push_field(col_name);
            }
        } else {
            output_headers.push_field(b"doc");
        }

        output_headers.push_field(b"token_count");
        output_headers.push_field(b"distinct_token_count");

        wtr.write_byte_record(&output_headers)?;

        vocab.for_each_doc_level_record(|r| wtr.write_byte_record(r))?;
    } else if args.cmd_corpus {
        let headers: [&[u8]; 4] = [
            b"doc_count",
            b"token_count",
            b"distinct_token_count",
            b"average_doc_len",
        ];
        wtr.write_record(headers)?;

        wtr.write_record([
            vocab.doc_count().to_string().as_bytes(),
            vocab.token_count.to_string().as_bytes(),
            vocab.distinct_token_count().to_string().as_bytes(),
            vocab.average_doc_len().to_string().as_bytes(),
        ])?;
    }

    Ok(wtr.flush()?)
}

type Document = ByteRecord;
type Token = Vec<u8>;
type TokenID = usize;

#[derive(Debug)]
struct TokenStats {
    gf: u64,
    df: u64,
    text: Token,
}

impl TokenStats {
    fn idf(&self, n: usize) -> f64 {
        (n as f64 / self.df as f64).ln()
    }

    // NOTE: this metric does not "log" the idf
    fn gfidf(&self, n: usize) -> f64 {
        self.gf as f64 * (n as f64 / self.df as f64)
    }

    fn pigeon(&self, n: usize) -> f64 {
        let n = n as f64;

        let expected = n - n * ((n - 1.0) / n).powf(self.gf as f64);

        // NOTE: the paper is not completely clear regarding what should
        // be the divisor and the numerator here. I aligned it to the graph
        // page 75, and with the gfidf metric.
        expected / self.df as f64
    }
}

impl From<Token> for TokenStats {
    fn from(value: Token) -> Self {
        TokenStats {
            gf: 0,
            df: 0,
            text: value,
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
enum TfWeighting {
    Count,
    Binary,
    Ratio,
    LogNormal,
}

impl TfWeighting {
    #[inline]
    fn compute(&self, count: u64, doc_len: usize) -> f64 {
        match self {
            Self::Count => count as f64,
            Self::Binary => 1.0,
            Self::Ratio => count as f64 / doc_len as f64,
            Self::LogNormal => (1.0 + count as f64).ln(),
        }
    }
}

impl TryFrom<String> for TfWeighting {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(match value.as_str() {
            "count" => Self::Count,
            "binary" => Self::Binary,
            "ratio" => Self::Ratio,
            "log-normal" => Self::LogNormal,
            _ => {
                return Err(format!(
                    "unsupported significance --tf-weight \"{}\"",
                    &value
                ))
            }
        })
    }
}

#[derive(Debug)]
struct DocumentTokenStats {
    tf: u64,
}

impl DocumentTokenStats {
    // NOTE: fancy idf log(1 + (N - tf + 0.5) / (tf + 0.5)) is the same as log(N / tf)
    // References:
    //   - https://fr.wikipedia.org/wiki/Okapi_BM25
    //   - https://kmwllc.com/index.php/2020/03/20/understanding-tf-idf-and-bm-25/
    fn bm25(&self, idf: f64, dl: usize, adl: f64, k1: f64, b: f64) -> f64 {
        let tf = self.tf as f64;

        // NOTE: Lucene does not multiply by (k1 + 1) because it
        // does not affect order when scoring.
        let numerator = tf * (k1 + 1.0);
        let denominator = tf + k1 * (1.0 - b + (b * (dl as f64 / adl)));

        idf * (numerator / denominator)
    }

    // NOTE: this function is a scaled version of the contingency matrix first cell
    // fn chi2(&self, doc_len: usize, expected: f64) -> f64 {
    //     let tf = self.tf as f64;

    //     ((tf / doc_len as f64) - expected).powi(2) / expected
    // }
}

#[derive(Default, Debug)]
struct DocumentStats {
    tokens: HashMap<TokenID, DocumentTokenStats>,
    len: usize,
}

impl DocumentStats {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, token_id: TokenID) -> bool {
        self.len += 1;

        match self.tokens.entry(token_id) {
            Entry::Occupied(mut entry) => {
                entry.get_mut().tf += 1;
                false
            }
            Entry::Vacant(entry) => {
                entry.insert(DocumentTokenStats { tf: 1 });
                true
            }
        }
    }

    fn doc_len(&self) -> usize {
        self.len
    }
}

#[derive(Default, Debug)]
struct Vocabulary {
    token_ids: HashMap<Token, TokenID>,
    tokens: Vec<TokenStats>,
    token_count: usize,
    documents: ClusteredInsertHashmap<Document, DocumentStats>,
}

impl Vocabulary {
    fn new() -> Self {
        Self::default()
    }

    fn doc_count(&self) -> usize {
        self.documents.len()
    }

    fn distinct_token_count(&self) -> usize {
        self.tokens.len()
    }

    fn average_doc_len(&self) -> f64 {
        self.token_count as f64 / self.doc_count() as f64
    }

    fn add(&mut self, document: Document, token: Token) {
        self.token_count += 1;

        let next_id = self.token_ids.len();

        let token_id = match self.token_ids.entry(token.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(next_id);
                self.tokens.push(TokenStats::from(token));

                next_id
            }
            Entry::Occupied(entry) => *entry.get(),
        };

        let token_stats = &mut self.tokens[token_id];
        token_stats.gf += 1;

        let mut token_was_added = false;

        let doc_was_inserted = self.documents.insert_with_or_else(
            document,
            || {
                let mut doc_stats = DocumentStats::new();
                doc_stats.add(token_id);
                doc_stats
            },
            |doc_stats| {
                token_was_added = doc_stats.add(token_id);
            },
        );

        if token_was_added || doc_was_inserted {
            token_stats.df += 1;
        }
    }

    fn for_each_doc_level_record<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        let mut record = ByteRecord::new();

        for (doc, doc_stats) in self.documents.into_iter() {
            record.clear();

            for cell in doc.into_iter() {
                record.push_field(cell);
            }

            record.push_field(doc_stats.doc_len().to_string().as_bytes());
            record.push_field(doc_stats.tokens.len().to_string().as_bytes());

            callback(&record)?;
        }

        Ok(())
    }

    fn for_each_token_level_record<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        let n = self.doc_count();

        if n == 0 {
            return Ok(());
        }

        let mut record = ByteRecord::new();

        for stats in self.tokens.into_iter() {
            record.clear();
            record.push_field(&stats.text);
            record.push_field(stats.gf.to_string().as_bytes());
            record.push_field(stats.df.to_string().as_bytes());
            record.push_field((stats.df as f64 / n as f64).to_string().as_bytes());
            record.push_field(stats.idf(n).to_string().as_bytes());
            record.push_field(stats.gfidf(n).to_string().as_bytes());
            record.push_field(stats.pigeon(n).to_string().as_bytes());

            callback(&record)?;
        }

        Ok(())
    }

    fn for_each_doc_token_level_record<F, E>(
        self,
        k1: f64,
        b: f64,
        tf_weighting: TfWeighting,
        chi2_significance: Option<f64>,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        let n = self.doc_count();

        if n == 0 {
            return Ok(());
        }

        let average_doc_len = self.average_doc_len();

        // Aggregating stats for bm25 and chi2
        let mut record = ByteRecord::new();

        for (doc, doc_stats) in self.documents.into_iter() {
            let doc_len = doc_stats.doc_len();

            for (token_id, doc_token_stats) in doc_stats.tokens {
                record.clear();

                let token_stats = &self.tokens[token_id];

                let expected_tf =
                    (token_stats.gf as usize * doc_len) as f64 / self.token_count as f64;

                let chi2 = compute_chi2(
                    token_stats.gf as usize,
                    doc_len,
                    doc_token_stats.tf as usize,
                    self.token_count,
                );

                let under_represented = (doc_token_stats.tf as f64) < expected_tf;

                if let Some(level) = chi2_significance {
                    if under_represented || chi2 < level {
                        continue;
                    }
                }

                let tf = tf_weighting.compute(doc_token_stats.tf, doc_len);
                let idf = token_stats.idf(n);

                for cell in doc.iter() {
                    record.push_field(cell);
                }

                record.push_field(&token_stats.text);
                record.push_field(tf.to_string().as_bytes());
                record.push_field(expected_tf.to_string().as_bytes());
                record.push_field((tf * idf).to_string().as_bytes());
                record.push_field(
                    doc_token_stats
                        .bm25(idf, doc_len, average_doc_len, k1, b)
                        .to_string()
                        .as_bytes(),
                );

                if !under_represented {
                    record.push_field(chi2.to_string().as_bytes());
                } else {
                    record.push_field(b"");
                }

                callback(&record)?;
            }
        }

        Ok(())
    }
}

#[inline]
fn compute_pmi(x: usize, y: usize, xy: usize, n: usize) -> f64 {
    // NOTE: (xy / n) / ((x / n) * (y / n)) = (xy * z) / (x * y)
    ((xy * n) as f64 / (x * y) as f64).log2()
}

#[inline]
fn compute_npmi(xy: usize, n: usize, pmi: f64) -> f64 {
    // If probability is 1, then self-information is 0 and npmi must be 1, meaning full co-occurrence.
    if xy >= n {
        1.0
    } else {
        let p_xy = xy as f64 / n as f64;

        pmi / (-p_xy.log2())
    }
}

// #[inline]
// fn compute_simplified_chi2_and_g2(x: usize, y: usize, xy: usize, n: usize) -> (f64, f64) {
//     // This version does not take into account the full contingency matrix.
//     let observed = xy as f64;
//     let expected = x as f64 * y as f64 / n as f64;

//     (
//         (observed - expected).powi(2) / expected,
//         2.0 * observed * (observed / expected).ln(),
//     )
// }

// NOTE: see code in issue https://github.com/medialab/xan/issues/295
// NOTE: it is possible to approximate chi2 and G2 for co-occurrences by
// only computing the (observed_11, expected_11) part related to the first
// cell of the contingency matrix. This works very well for chi2, but
// is a little bit more fuzzy for G2.
fn compute_chi2_and_g2(x: usize, y: usize, xy: usize, n: usize) -> (f64, f64) {
    // This can be 0 if some item is present in all co-occurrences!
    let not_x = n - x;
    let not_y = n - y;

    let observed_11 = xy as f64;
    let observed_12 = (x - xy) as f64; // Is 0 if x only co-occurs with y
    let observed_21 = (y - xy) as f64; // Is 0 if y only co-occurs with x

    // NOTE: with few co-occurrences, self loops can produce a negative
    // outcome...
    let observed_22 = ((n + xy) as i64 - (x + y) as i64) as f64;

    let nf = n as f64;

    let expected_11 = (x * y) as f64 / nf; // Cannot be 0
    let expected_12 = (x * not_y) as f64 / nf;
    let expected_21 = (y * not_x) as f64 / nf;
    let expected_22 = (not_x * not_y) as f64 / nf;

    debug_assert!(
        observed_11 >= 0.0
            && observed_12 >= 0.0
            && observed_21 >= 0.0
            // && observed_22 >= 0.0
            && expected_11 >= 0.0
            && expected_12 >= 0.0
            && expected_21 >= 0.0
            && expected_22 >= 0.0
    );

    let chi2_11 = (observed_11 - expected_11).powi(2) / expected_11;
    let chi2_12 = (observed_12 - expected_12).powi(2) / expected_12;
    let chi2_21 = (observed_21 - expected_21).powi(2) / expected_21;
    let chi2_22 = (observed_22 - expected_22).powi(2) / expected_22;

    let g2_11 = observed_11 * (observed_11 / expected_11).ln();
    let g2_12 = if observed_12 == 0.0 {
        0.0
    } else {
        observed_12 * (observed_12 / expected_12).ln()
    };
    let g2_21 = if observed_21 == 0.0 {
        0.0
    } else {
        observed_21 * (observed_21 / expected_21).ln()
    };

    // NOTE: in the case when observed_22 is negative, I am not entirely
    // sure it is mathematically sound to clamp to 0. But since this case
    // is mostly useless, I will allow it...
    let g2_22 = if observed_22 <= 0.0 {
        0.0
    } else {
        observed_22 * (observed_22 / expected_22).ln()
    };

    let mut chi2 = chi2_11 + chi2_12 + chi2_21 + chi2_22;
    let mut g2 = 2.0 * (g2_11 + g2_12 + g2_21 + g2_22);

    // Dealing with degenerate cases that happen when the number
    // of co-occurrences is very low, or when some item dominates
    // the distribution.
    if chi2.is_nan() {
        chi2 = 0.0;
    }

    if chi2.is_infinite() {
        chi2 = chi2_11;
    }

    if g2.is_infinite() {
        g2 = g2_11;
    }

    (chi2, g2)
}

fn compute_chi2(x: usize, y: usize, xy: usize, n: usize) -> f64 {
    // This can be 0 if some item is present in all co-occurrences!
    let not_x = n - x;
    let not_y = n - y;

    let observed_11 = xy as f64;
    let observed_12 = (x - xy) as f64; // Is 0 if x only co-occurs with y
    let observed_21 = (y - xy) as f64; // Is 0 if y only co-occurs with x

    // NOTE: with few co-occurrences, self loops can produce a negative
    // outcome...
    let observed_22 = ((n + xy) as i64 - (x + y) as i64) as f64;

    let nf = n as f64;

    let expected_11 = (x * y) as f64 / nf; // Cannot be 0
    let expected_12 = (x * not_y) as f64 / nf;
    let expected_21 = (y * not_x) as f64 / nf;
    let expected_22 = (not_x * not_y) as f64 / nf;

    debug_assert!(
        observed_11 >= 0.0
            && observed_12 >= 0.0
            && observed_21 >= 0.0
            // && observed_22 >= 0.0
            && expected_11 >= 0.0
            && expected_12 >= 0.0
            && expected_21 >= 0.0
            && expected_22 >= 0.0
    );

    let chi2_11 = (observed_11 - expected_11).powi(2) / expected_11;
    let chi2_12 = (observed_12 - expected_12).powi(2) / expected_12;
    let chi2_21 = (observed_21 - expected_21).powi(2) / expected_21;
    let chi2_22 = (observed_22 - expected_22).powi(2) / expected_22;

    let mut chi2 = chi2_11 + chi2_12 + chi2_21 + chi2_22;

    // Dealing with degenerate cases that happen when the number
    // of co-occurrences is very low, or when some item dominates
    // the distribution.
    if chi2.is_nan() {
        chi2 = 0.0;
    }

    if chi2.is_infinite() {
        chi2 = chi2_11;
    }

    chi2
}

fn compute_g2(x: usize, y: usize, xy: usize, n: usize) -> f64 {
    // This can be 0 if some item is present in all co-occurrences!
    let not_x = n - x;
    let not_y = n - y;

    let observed_11 = xy as f64;
    let observed_12 = (x - xy) as f64; // Is 0 if x only co-occurs with y
    let observed_21 = (y - xy) as f64; // Is 0 if y only co-occurs with x

    // NOTE: with few co-occurrences, self loops can produce a negative
    // outcome...
    let observed_22 = ((n + xy) as i64 - (x + y) as i64) as f64;

    let nf = n as f64;

    let expected_11 = (x * y) as f64 / nf; // Cannot be 0
    let expected_12 = (x * not_y) as f64 / nf;
    let expected_21 = (y * not_x) as f64 / nf;
    let expected_22 = (not_x * not_y) as f64 / nf;

    debug_assert!(
        observed_11 >= 0.0
            && observed_12 >= 0.0
            && observed_21 >= 0.0
            // && observed_22 >= 0.0
            && expected_11 >= 0.0
            && expected_12 >= 0.0
            && expected_21 >= 0.0
            && expected_22 >= 0.0
    );

    let g2_11 = observed_11 * (observed_11 / expected_11).ln();
    let g2_12 = if observed_12 == 0.0 {
        0.0
    } else {
        observed_12 * (observed_12 / expected_12).ln()
    };
    let g2_21 = if observed_21 == 0.0 {
        0.0
    } else {
        observed_21 * (observed_21 / expected_21).ln()
    };

    // NOTE: in the case when observed_22 is negative, I am not entirely
    // sure it is mathematically sound to clamp to 0. But since this case
    // is mostly useless, I will allow it...
    let g2_22 = if observed_22 <= 0.0 {
        0.0
    } else {
        observed_22 * (observed_22 / expected_22).ln()
    };

    let mut g2 = 2.0 * (g2_11 + g2_12 + g2_21 + g2_22);

    // Dealing with degenerate cases that happen when the number
    // of co-occurrences is very low, or when some item dominates
    // the distribution.

    if g2.is_infinite() {
        g2 = g2_11;
    }

    g2
}

#[derive(Debug)]
struct CooccurrenceTokenEntry {
    token: Rc<Token>,
    gcf: usize,
    cooc: ClusteredInsertHashmap<TokenID, usize>,
}

impl CooccurrenceTokenEntry {
    fn new(token: Rc<Token>) -> Self {
        Self {
            token,
            gcf: 0,
            cooc: ClusteredInsertHashmap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum CooccurrenceMode {
    Symmetrical,
    Forward,
    Full,
}

impl CooccurrenceMode {
    fn is_symmetrical(&self) -> bool {
        matches!(self, Self::Symmetrical)
    }

    fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}

#[derive(Default, Debug)]
struct Cooccurrences {
    token_ids: HashMap<Rc<Token>, TokenID>,
    token_entries: Vec<CooccurrenceTokenEntry>,
    cooccurrences_count: usize,
}

impl Cooccurrences {
    fn register_token(&mut self, token: Rc<Token>) -> TokenID {
        match self.token_ids.entry(token.clone()) {
            Entry::Occupied(entry) => {
                let id = *entry.get();
                id
            }
            Entry::Vacant(entry) => {
                let id = self.token_entries.len();
                let token_entry = CooccurrenceTokenEntry::new(token);
                self.token_entries.push(token_entry);
                entry.insert(id);
                id
            }
        }
    }

    fn add_cooccurrence(
        &mut self,
        mode: CooccurrenceMode,
        mut source: TokenID,
        mut target: TokenID,
    ) {
        if mode.is_symmetrical() && source > target {
            (source, target) = (target, source);
        }

        self.cooccurrences_count += 1;

        let source_entry = &mut self.token_entries[source];

        source_entry
            .cooc
            .insert_with_or_else(target, || 1, |count| *count += 1);

        source_entry.gcf += 1;

        // Do not overcount self-links
        if source == target {
            return;
        }

        let target_entry = &mut self.token_entries[target];

        if mode.is_full() {
            target_entry
                .cooc
                .insert_with_or_else(source, || 1, |count| *count += 1);
        }

        target_entry.gcf += 1;
    }

    fn for_each_cooc_record<F, E>(
        self,
        min_count: usize,
        chi2_significance: Option<f64>,
        g2_significance: Option<f64>,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        let mut csv_record = ByteRecord::new();
        let n = self.cooccurrences_count;

        for source_entry in self.token_entries.iter() {
            let x = source_entry.gcf;

            for (target_id, count) in source_entry.cooc.iter() {
                if *count < min_count {
                    continue;
                }

                let target_entry = &self.token_entries[*target_id];

                let y = target_entry.gcf;
                let xy = *count;

                let expected = (x * y) as f64 / n as f64;
                let under_represented = (*count as f64) < expected;

                // chi2/G2 computations
                let (chi2, g2) = compute_chi2_and_g2(x, y, xy, n);

                if let Some(level) = chi2_significance {
                    if under_represented || chi2 < level {
                        continue;
                    }
                }

                if let Some(level) = g2_significance {
                    if g2 < level {
                        continue;
                    }
                }

                // PMI-related computations
                let pmi = compute_pmi(x, y, xy, n);
                let npmi = compute_npmi(xy, n, pmi);

                csv_record.clear();
                csv_record.push_field(&source_entry.token);
                csv_record.push_field(&target_entry.token);
                csv_record.push_field(count.to_string().as_bytes());
                csv_record.push_field(expected.to_string().as_bytes());

                if !under_represented {
                    csv_record.push_field(chi2.to_string().as_bytes());
                } else {
                    csv_record.push_field(b"");
                }

                csv_record.push_field(g2.to_string().as_bytes());
                csv_record.push_field(pmi.to_string().as_bytes());
                csv_record.push_field(npmi.to_string().as_bytes());

                callback(&csv_record)?;
            }
        }

        Ok(())
    }

    // NOTE: currently we avoid self loops because they are fiddly
    fn for_each_distrib_cooc_record<F, E>(self, min_count: usize, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        #[derive(Default)]
        struct Metrics {
            pmi: f64,
            g2: f64,
        }

        let mut csv_record = ByteRecord::new();
        let n = self.cooccurrences_count;

        let mut sums: Vec<Metrics> = Vec::with_capacity(self.token_entries.len());

        for (source_id, source_entry) in self.token_entries.iter().enumerate() {
            let x = source_entry.gcf;

            let mut sum = Metrics::default();

            for (target_id, count) in source_entry.cooc.iter() {
                if source_id == *target_id {
                    continue;
                }

                let target_entry = &self.token_entries[*target_id];

                let y = target_entry.gcf;
                let xy = *count;

                // PMI-related computations
                let pmi = compute_pmi(x, y, xy, n);

                if pmi > 0.0 {
                    sum.pmi += pmi;
                }

                // G2 computations
                let g2 = compute_g2(x, y, xy, n);

                if g2 > 0.0 {
                    sum.g2 += g2;
                }
            }

            sums.push(sum);
        }

        for (source_id, source_entry) in self.token_entries.iter().enumerate() {
            for (target_id, source_target_count) in source_entry.cooc.iter() {
                if source_id == *target_id || *source_target_count < min_count {
                    continue;
                }

                let target_entry = &self.token_entries[*target_id];

                // We do both entries at once and we optimize by intersection length
                match source_entry.cooc.len().cmp(&target_entry.cooc.len()) {
                    // Ids serve as tie-breaker if needed
                    Ordering::Equal if source_id > *target_id => continue,
                    Ordering::Greater => continue,
                    _ => (),
                };

                debug_assert!(source_entry.cooc.len() <= target_entry.cooc.len());

                let mut min_pmi_sum = 0.0;
                let mut min_g2_sum = 0.0;

                for (other_id, source_other_count) in source_entry.cooc.iter() {
                    if source_id == *other_id {
                        continue;
                    }

                    let target_other_count = match target_entry.cooc.get(other_id) {
                        Some(c) => c,
                        None => continue,
                    };

                    let other_entry = &self.token_entries[*other_id];

                    let pmi_source_other =
                        compute_pmi(source_entry.gcf, other_entry.gcf, *source_other_count, n);
                    let pmi_target_other =
                        compute_pmi(target_entry.gcf, other_entry.gcf, *target_other_count, n);

                    if pmi_source_other > 0.0 && pmi_target_other > 0.0 {
                        min_pmi_sum += pmi_source_other.min(pmi_target_other);
                    }

                    let g2_source_other =
                        compute_g2(source_entry.gcf, other_entry.gcf, *source_other_count, n);
                    let g2_target_other =
                        compute_g2(target_entry.gcf, other_entry.gcf, *target_other_count, n);

                    if g2_source_other > 0.0 && g2_target_other > 0.0 {
                        min_g2_sum += g2_source_other.min(g2_target_other);
                    }
                }

                // No intersection, we skip this edge
                if min_pmi_sum == 0.0 && min_g2_sum == 0.0 {
                    continue;
                }

                let source_sum = &sums[source_id];

                let expected = (source_entry.gcf * target_entry.gcf) as f64 / n as f64;

                let sd_i = min_pmi_sum / source_sum.pmi;
                let sd_g2 = min_g2_sum / source_sum.g2;

                csv_record.clear();
                csv_record.push_field(&source_entry.token);
                csv_record.push_field(&target_entry.token);
                csv_record.push_field(source_target_count.to_string().as_bytes());
                csv_record.push_field(expected.to_string().as_bytes());
                csv_record.push_field(sd_i.to_string().as_bytes());
                csv_record.push_field(sd_g2.to_string().as_bytes());

                callback(&csv_record)?;

                let target_sum = &sums[*target_id];

                let sd_i = min_pmi_sum / target_sum.pmi;
                let sd_g2 = min_g2_sum / target_sum.g2;

                csv_record.clear();
                csv_record.push_field(&target_entry.token);
                csv_record.push_field(&source_entry.token);
                csv_record.push_field(source_target_count.to_string().as_bytes());
                csv_record.push_field(expected.to_string().as_bytes());
                csv_record.push_field(sd_i.to_string().as_bytes());
                csv_record.push_field(sd_g2.to_string().as_bytes());

                callback(&csv_record)?;
            }
        }

        Ok(())
    }

    fn for_each_specificity_record<F, E>(
        self,
        min_count: usize,
        g2_significance: Option<f64>,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&ByteRecord) -> Result<(), E>,
    {
        let mut csv_record = ByteRecord::new();
        let n = self.cooccurrences_count;

        let g2_significance = g2_significance.unwrap_or(3.84);

        for source_entry in self.token_entries.iter() {
            if source_entry.gcf < min_count {
                continue;
            }

            csv_record.clear();

            let mut statistically_significant: usize = 0;

            for (target_id, source_target_count) in source_entry.cooc.iter() {
                let target_entry = &self.token_entries[*target_id];

                let g2 = compute_g2(source_entry.gcf, target_entry.gcf, *source_target_count, n);

                // 5% test
                if g2 >= g2_significance {
                    statistically_significant += 1;
                }
            }

            let lgl = statistically_significant as f64 / n as f64;

            csv_record.push_field(&source_entry.token);
            csv_record.push_field(source_entry.gcf.to_string().as_bytes());
            csv_record.push_field(lgl.to_string().as_bytes());

            callback(&csv_record)?;
        }

        Ok(())
    }
}
