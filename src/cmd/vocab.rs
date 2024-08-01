use std::collections::{hash_map::Entry, HashMap};

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

// TODO: --sorted if sorted on document to speed up and avoid lookups

static USAGE: &str = "
Build a vocabulary over tokenized documents.

TODO...

Usage:
    xan vocab <doc-columns> <token-column> [options] [<input>]

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
    let headers = rdr.byte_headers()?;

    let sel = rconf.selection(headers)?;
    let token_pos = Config::new(&None)
        .select(args.arg_token_column)
        .single_selection(headers)?;

    let mut vocab = Vocabulary::new();

    let mut record = csv::ByteRecord::new();

    while rdr.read_byte_record(&mut record)? {
        let document: Document = sel.select(&record).map(|cell| cell.to_vec()).collect();
        let token: Token = record[token_pos].to_vec();

        vocab.add(document, token);
    }

    let mut wtr = Config::new(&args.flag_output).writer()?;

    wtr.write_record(["id", "token", "tf", "df", "idf"])?;

    vocab.for_each_token_level_record(|r| wtr.write_byte_record(r))?;

    Ok(wtr.flush()?)
}

type Document = Vec<Vec<u8>>;
type Token = Vec<u8>;

#[derive(Debug)]
struct TokenStats {
    tf: u64,
    df: u64,
}

impl TokenStats {
    fn idf(&self, n: u64) -> f64 {
        (n as f64 / self.df as f64).ln()
    }
}

#[derive(Debug)]
struct DocumentTokenStats {
    tf: u64,
}

#[derive(Default, Debug)]
struct DocumentStats {
    tokens: HashMap<Token, DocumentTokenStats>,
}

impl DocumentStats {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, token: Token) -> bool {
        match self.tokens.entry(token) {
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
    tokens: HashMap<Token, TokenStats>,
    documents: HashMap<Document, DocumentStats>,
}

// TODO: don't clone token and make the tokens hashmap store references?
impl Vocabulary {
    fn new() -> Self {
        Self::default()
    }

    fn doc_count(&self) -> u64 {
        self.documents.len() as u64
    }

    fn add(&mut self, document: Document, token: Token) {
        let token_was_added = match self.documents.entry(document) {
            Entry::Vacant(entry) => {
                let mut stats = DocumentStats::new();
                let added = stats.add(token.clone());

                entry.insert(stats);

                added
            }
            Entry::Occupied(mut entry) => entry.get_mut().add(token.clone()),
        };

        self.tokens
            .entry(token)
            .and_modify(|stats| {
                if token_was_added {
                    stats.df += 1;
                }
                stats.tf += 1;
            })
            .or_insert(TokenStats { tf: 1, df: 1 });
    }

    fn for_each_token_level_record<F, E>(self, mut callback: F) -> Result<(), E>
    where
        F: FnMut(&csv::ByteRecord) -> Result<(), E>,
    {
        let mut record = csv::ByteRecord::new();
        let n = self.doc_count();

        for (i, (token, stats)) in self.tokens.into_iter().enumerate() {
            record.clear();
            record.push_field(i.to_string().as_bytes());
            record.push_field(&token);
            record.push_field(stats.tf.to_string().as_bytes());
            record.push_field(stats.df.to_string().as_bytes());
            record.push_field(stats.idf(n).to_string().as_bytes());

            callback(&record)?;
        }

        Ok(())
    }
}
