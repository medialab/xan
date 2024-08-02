use std::collections::{hash_map::Entry, HashMap};

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

// TODO: --sorted if sorted on document to speed up and avoid lookups,
// or rely on caching last key instead, but we can avoid the doc hashmap
// if sorted!
// TODO: filters on df because with --doc you cannot filter on this?
// TODO: maybe option to explode for perf reasons?
// TODO: add bm25, chi2

static USAGE: &str = "
Build a vocabulary over tokenized documents.

TODO...

Usage:
    xan vocab <doc-columns> <token-column> [options] [<input>]

vocab options:
    -D, --doc              Compute doc-level statistics for tokens instead
                           of token-level statistics.

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
    arg_input: Option<String>,
    arg_doc_columns: SelectColumns,
    arg_token_column: SelectColumns,
    flag_doc: bool,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.arg_doc_columns);

    let mut rdr = rconf.reader()?;
    let headers = rdr.byte_headers()?.clone();

    let sel = rconf.selection(&headers)?;
    let token_pos = Config::new(&None)
        .select(args.arg_token_column)
        .single_selection(&headers)?;

    let mut vocab = Vocabulary::new();

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        let document: Document = sel.select(&record).map(|cell| cell.to_vec()).collect();
        let token: Token = record[token_pos].to_vec();

        vocab.add(document, token);
    }

    let mut wtr = Config::new(&args.flag_output).writer()?;

    if !args.flag_doc {
        wtr.write_record([b"id", &headers[token_pos], b"tf", b"df", b"idf"])?;
        vocab.for_each_token_level_record(|r| wtr.write_byte_record(r))?;
    } else {
        let mut output_headers = csv::ByteRecord::new();

        for col_name in sel.select(&headers) {
            output_headers.push_field(col_name);
        }

        output_headers.push_field(&headers[token_pos]);
        output_headers.push_field(b"tf");
        output_headers.push_field(b"tfidf");

        wtr.write_byte_record(&output_headers)?;
        vocab.for_each_doc_token_level_record(|r| wtr.write_byte_record(r))?;
    }

    Ok(wtr.flush()?)
}

type Document = Vec<Vec<u8>>;
type Token = Vec<u8>;
type TokenID = usize;

#[derive(Debug)]
struct TokenStats {
    tf: u64,
    df: u64,
    text: Token,
}

impl TokenStats {
    fn idf(&self, n: u64) -> f64 {
        (n as f64 / self.df as f64).ln()
    }
}

impl From<Token> for TokenStats {
    fn from(value: Token) -> Self {
        TokenStats {
            tf: 0,
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
}

#[derive(Default, Debug)]
struct DocumentStats {
    tokens: HashMap<TokenID, DocumentTokenStats>,
}

impl DocumentStats {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, token_id: TokenID) -> bool {
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
}

#[derive(Default, Debug)]
struct Vocabulary {
    token_ids: HashMap<Token, TokenID>,
    tokens: Vec<TokenStats>,
    documents: HashMap<Document, DocumentStats>,
}

impl Vocabulary {
    fn new() -> Self {
        Self::default()
    }

    fn doc_count(&self) -> u64 {
        self.documents.len() as u64
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
        token_stats.tf += 1;

        let token_was_added_to_doc = match self.documents.entry(document) {
            Entry::Vacant(entry) => {
                let mut stats = DocumentStats::new();
                let added = stats.add(token_id);

                entry.insert(stats);

                added
            }
            Entry::Occupied(mut entry) => entry.get_mut().add(token_id),
        };

        if token_was_added_to_doc {
            token_stats.df += 1;
        }
    }

    fn for_each_token_level_record<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&csv::ByteRecord) -> Result<(), E>,
    {
        let mut record = csv::ByteRecord::new();
        let n = self.doc_count();

        for (token, token_id) in self.token_ids.into_iter() {
            let stats = &self.tokens[token_id];

            record.clear();
            record.push_field(token_id.to_string().as_bytes());
            record.push_field(&token);
            record.push_field(stats.tf.to_string().as_bytes());
            record.push_field(stats.df.to_string().as_bytes());
            record.push_field(stats.idf(n).to_string().as_bytes());

            callback(&record)?;
        }

        Ok(())
    }

    fn for_each_doc_token_level_record<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&csv::ByteRecord) -> Result<(), E>,
    {
        let mut record = csv::ByteRecord::new();
        let n = self.doc_count();

        for (doc, doc_stats) in self.documents.into_iter() {
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

                callback(&record)?;
            }
        }

        Ok(())
    }
}
