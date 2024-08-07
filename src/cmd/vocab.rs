use std::collections::{hash_map::Entry, HashMap};

use crate::collections::SortedInsertHashmap;
use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

// TODO: filters on df because with some stats you cannot filter on this?
// TODO: maybe option to explode for perf reasons?
// TODO: add chi2

static USAGE: &str = "
Compute vocabulary statistics over tokenized documents. Those documents
must be given as a CSV with one or more column representing a document key
and a column containing single tokens like so (typically an output from
the \"tokenize\" command):

doc,token
1,the
1,cat
1,eats
2,hello

This command can compute 4 kinds of differents vocabulary statistics:

1. corpus-level statistics (using the \"corpus\" subcommand):
    - doc_count: number of documents in the corpus
    - token_count: number of distinct tokens in the corpus
    - average_doc_len: average number of tokens per document

2. token-level statistics (using the \"token\" subcommand):
    - (token): some distinct token (the column will be named like the input)
    - gf: global frequency of the token across corpus
    - df: document frequency of the token
    - idf: inverse document frequency of the token
    - gfidf: global frequency * idf for the token
    - pigeonhole: ratio between df and expected df in random distribution

3. doc-level statistics (using the \"doc\" subcommand):
    - (*doc): columns representing the document (named like the input)
    - token_count: total number of tokens in document
    - distinct_token_count: number of distinct tokens in document

4. doc-token-level statistics (using the \"doc-token\" subcommand):
    - (*doc): columns representing the document (named like the input)
    - (token): some distinct documnet token (the column will be named like the input)
    - tf: term frequency for the token in the document
    - tfidf: term frequency * idf for the token in the document
    - bm25: BM25 score for the token in the document

Usage:
    xan vocab corpus <doc-cols> <token-col> [options] [<input>]
    xan vocab token <doc-cols> <token-col> [options] [<input>]
    xan vocab doc <doc-cols> <token-col> [options] [<input>]
    xan vocab doc-token <doc-cols> <token-col> [options] [<input>]
    xan vocab --help

vocab doc-token options:
    --k1-value <value>     \"k1\" factor for BM25 computation. [default: 1.2]
    --b-value <value>      \"b\" factor for BM25 computation. [default: 0.75]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will NOT be included
                           in the frequency table. Additionally, the 'field'
                           column will be 1-based indices instead of header
                           names.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Clone, Deserialize)]
struct Args {
    cmd_token: bool,
    cmd_doc: bool,
    cmd_doc_token: bool,
    cmd_corpus: bool,
    arg_input: Option<String>,
    arg_doc_cols: SelectColumns,
    arg_token_col: SelectColumns,
    flag_k1_value: f64,
    flag_b_value: f64,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_doc_cols);

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let sel = rconf.selection(&headers)?;
    let token_pos = Config::new(&None)
        .select(args.arg_token_col)
        .single_selection(&headers)?;

    let mut vocab = Vocabulary::new();

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        let document: Document = sel.select(&record).map(|cell| cell.to_vec()).collect();
        let token: Token = record[token_pos].to_vec();

        vocab.add(document, token);
    }

    let mut wtr = Config::new(&args.flag_output).writer()?;

    if args.cmd_token {
        wtr.write_record([
            &headers[token_pos],
            b"gf",
            b"df",
            b"idf",
            b"gfidf",
            b"pigeonhole",
        ])?;
        vocab.for_each_token_level_record(|r| wtr.write_byte_record(r))?;
    } else if args.cmd_doc_token {
        let mut output_headers = csv::ByteRecord::new();

        for col_name in sel.select(&headers) {
            output_headers.push_field(col_name);
        }

        output_headers.push_field(&headers[token_pos]);
        output_headers.push_field(b"tf");
        output_headers.push_field(b"tfidf");
        output_headers.push_field(b"bm25");

        wtr.write_byte_record(&output_headers)?;
        vocab.for_each_doc_token_level_record(args.flag_k1_value, args.flag_b_value, |r| {
            wtr.write_byte_record(r)
        })?;
    } else if args.cmd_doc {
        let mut output_headers = csv::ByteRecord::new();

        for col_name in sel.select(&headers) {
            output_headers.push_field(col_name);
        }
        output_headers.push_field(b"token_count");
        output_headers.push_field(b"distinct_token_count");

        wtr.write_byte_record(&output_headers)?;

        vocab.for_each_doc_level_record(|r| wtr.write_byte_record(r))?;
    } else if args.cmd_corpus {
        let headers: [&[u8]; 3] = [b"doc_count", b"token_count", b"average_doc_len"];
        wtr.write_record(headers)?;
        wtr.write_record([
            vocab.doc_count().to_string().as_bytes(),
            vocab.token_count().to_string().as_bytes(),
            vocab.average_doc_len().to_string().as_bytes(),
        ])?;
    }

    Ok(wtr.flush()?)
}

type Document = Vec<Vec<u8>>;
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

    fn gfidf(&self, n: usize) -> f64 {
        self.gf as f64 * self.idf(n)
    }

    fn pigeonhole(&self, n: usize) -> f64 {
        let n = n as f64;

        let expected = n - (n * ((n - 1.0) / n).powf(self.gf as f64));

        self.df as f64 / expected
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

#[derive(Debug)]
struct DocumentTokenStats {
    tf: u64,
}

impl DocumentTokenStats {
    fn tfidf(&self, idf: f64) -> f64 {
        self.tf as f64 * idf
    }

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
    documents: SortedInsertHashmap<Document, DocumentStats>,
}

impl Vocabulary {
    fn new() -> Self {
        Self::default()
    }

    fn doc_count(&self) -> usize {
        self.documents.len()
    }

    fn token_count(&self) -> usize {
        self.tokens.len()
    }

    fn average_doc_len(&self) -> f64 {
        let doc_len_sum: usize = self.documents.values().map(|s| s.doc_len()).sum();
        doc_len_sum as f64 / self.doc_count() as f64
    }

    fn add(&mut self, document: Document, token: Token) {
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
            |mut doc_stats| {
                token_was_added = doc_stats.add(token_id);
            },
        );

        if token_was_added || doc_was_inserted {
            token_stats.df += 1;
        }
    }

    fn for_each_doc_level_record<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&csv::ByteRecord) -> Result<(), E>,
    {
        let mut record = csv::ByteRecord::new();

        for (doc, doc_stats) in self.documents.into_iter() {
            record.clear();

            for cell in doc {
                record.push_field(&cell);
            }

            record.push_field(doc_stats.doc_len().to_string().as_bytes());
            record.push_field(doc_stats.tokens.len().to_string().as_bytes());

            callback(&record)?;
        }

        Ok(())
    }

    fn for_each_token_level_record<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&csv::ByteRecord) -> Result<(), E>,
    {
        let n = self.doc_count();

        if n == 0 {
            return Ok(());
        }

        let mut record = csv::ByteRecord::new();

        for (token, token_id) in self.token_ids.into_iter() {
            let stats = &self.tokens[token_id];

            record.clear();
            record.push_field(&token);
            record.push_field(stats.gf.to_string().as_bytes());
            record.push_field(stats.df.to_string().as_bytes());
            record.push_field(stats.idf(n).to_string().as_bytes());
            record.push_field(stats.gfidf(n).to_string().as_bytes());
            record.push_field(stats.pigeonhole(n).to_string().as_bytes());

            callback(&record)?;
        }

        Ok(())
    }

    fn for_each_doc_token_level_record<F, E>(
        self,
        k1: f64,
        b: f64,
        mut callback: F,
    ) -> Result<(), E>
    where
        F: FnMut(&csv::ByteRecord) -> Result<(), E>,
    {
        let n = self.doc_count();

        if n == 0 {
            return Ok(());
        }

        // Aggregating average doc lengths for BM25
        let average_doc_len = self.average_doc_len();

        let mut record = csv::ByteRecord::new();

        for (doc, doc_stats) in self.documents.into_iter() {
            let doc_len = doc_stats.doc_len();

            for (token_id, doc_token_stats) in doc_stats.tokens {
                record.clear();

                let token_stats = &self.tokens[token_id];

                let idf = token_stats.idf(n);

                for cell in doc.iter() {
                    record.push_field(cell);
                }

                record.push_field(&token_stats.text);
                record.push_field(doc_token_stats.tf.to_string().as_bytes());
                record.push_field(doc_token_stats.tfidf(idf).to_string().as_bytes());
                record.push_field(
                    doc_token_stats
                        .bm25(idf, doc_len, average_doc_len, k1, b)
                        .to_string()
                        .as_bytes(),
                );

                callback(&record)?;
            }
        }

        Ok(())
    }
}
