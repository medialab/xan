use csv;
use tantivy::collector::{TopDocs, Count};
use tantivy::{Index, ReloadPolicy, schema::*, tokenizer::*, query::QueryParser};
use tempfile::TempDir;

use CliResult;
use config::{Config, Delimiter};
use select::SelectColumns;
use util;

static USAGE: &'static str = r#"
Filters CSV data by whether the given key words matches a row.

If the field matches, then the row is written to the output.

Currently supported languages for stemmer:

  * arabic
  * danish
  * dutch
  * english
  * finnish
  * french
  * german
  * greek
  * hungarian
  * italian
  * norwegian
  * portuguese
  * romanian
  * russian
  * spanish
  * swedish
  * tamil
  * turkish


Usage:
    xsv fullsearch [options] <keywords> [<input>]
    xsv fullsearch --help

fullsearch options:
    -s, --select <arg>     Select a subset of columns to search in.
                           See 'xsv select --help' for the format
                           details. This is provided here because piping 'xsv
                           select' into 'xsv fullsearch' will disable the use
                           of indexing.
    -l, --limit <arg>      Limit the fullsearch result to the N most relevant
                           items. Set to '0' to disable a limit.
                           [default: 0]
    --lang <arg>           Lang of the text to choose the correct stemmer.
                           [default: english]

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. (i.e., They are not searched, analyzed,
                           sliced, etc.)
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. (default: ,)
"#;

static LANGUAGES: &'static [&'static str] = &[
    "arabic",
    "danish",
    "dutch",
    "english",
    "finnish",
    "french",
    "german",
    "greek",
    "hungarian",
    "italian",
    "norwegian",
    "portuguese",
    "romanian",
    "russian",
    "spanish",
    "swedish",
    "tamil",
    "turkish"
];

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    arg_keywords: String,
    flag_select: SelectColumns,
    flag_limit: usize,
    flag_lang: String,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let keywords: Vec<_> = args.arg_keywords.split(",").collect();
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select);

    let mut rdr = rconfig.reader()?;
    let mut wtr = Config::new(&args.flag_output).writer()?;

    let headers = rdr.byte_headers()?.clone();
    let sel = rconfig.selection(&headers)?;
    let sel_len = sel.len();
    let mut it = sel.iter();
    let mut column_index = *it.next().unwrap();

    let lang: &str = &args.flag_lang;
    if !LANGUAGES.contains(&lang) {
        return fail!(format!("Unknown \"{}\" language found", lang));
    }
    let lang_stemmer = match lang {
        "arabic" => Language::Arabic,
        "danish" => Language::Danish,
        "dutch" => Language::Dutch,
        "english" => Language::English,
        "finnish" => Language::Finnish,
        "french" => Language::French,
        "german" => Language::German,
        "greek" => Language::Greek,
        "hungarian" => Language::Hungarian,
        "italian" => Language::Italian,
        "norwegian" => Language::Norwegian,
        "portuguese" => Language::Portuguese,
        "romanian" => Language::Romanian,
        "russian" => Language::Russian,
        "spanish" => Language::Spanish,
        "swedish" => Language::Swedish,
        "tamil" => Language::Tamil,
        "turkish" => Language::Turkish,
        &_ => Language::English,
    };

    let index_path = TempDir::new()?;

    let mut schema_builder = Schema::builder();
    let text_field_indexing = TextFieldIndexing::default()
        .set_tokenizer("custom")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let text_options = TextOptions::default()
        .set_indexing_options(text_field_indexing)
        .set_stored();
    let mut fields: Vec::<Field> = Vec::new();
    let mut searched_columns: Vec::<Field> = Vec::new();
    let mut nb_col = 0;
    for (i, _) in headers.into_iter().enumerate() {
        if i == column_index {
            let field = schema_builder.add_text_field(&i.to_string(), text_options.clone());
            fields.push(field);
            searched_columns.push(field);
            nb_col += 1;
            if nb_col < sel_len {
                column_index = *it.next().unwrap();
            }
        } else {
            fields.push(schema_builder.add_text_field(&i.to_string(), STORED));
        }
    }
    let schema = schema_builder.build();
    let index = Index::create_in_dir(&index_path, schema.clone())?;
    let mut index_writer = index.writer(50_000_000)?;
    let custom_tokenizer = TextAnalyzer::from(SimpleTokenizer)
        .filter(LowerCaser)
        .filter(Stemmer::new(lang_stemmer));
    index
        .tokenizers()
        .register("custom", custom_tokenizer);

    let mut record = csv::ByteRecord::new();
    while rdr.read_byte_record(&mut record)? {
        let mut doc = Document::default();
        for (i, header) in fields.clone().into_iter().enumerate() {
            doc.add_text(header, String::from_utf8(record[i].to_vec()).unwrap());
        }
        index_writer.add_document(doc)?;
    }
    index_writer.commit()?;

    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into()?;

    if !rconfig.no_headers {
        wtr.write_record(&headers)?;
    }
    let searcher = reader.searcher();
    let query_parser = QueryParser::for_index(&index, searched_columns);
    let query = query_parser.parse_query(&keywords.join(" "))?;
    let mut count = args.flag_limit;
    if count == 0 {
        count = searcher.search(&query, &Count)?;
    }
    if count != 0 {
        let top_docs = searcher.search(&query, &TopDocs::with_limit(count))?;
        for (_score, doc_address) in top_docs {
            let retrieved_doc = searcher.doc(doc_address)?;
            let mut row = Vec::new();
            for field in fields.clone().into_iter() {
                if let Some(field_value) = retrieved_doc.get_first(field) {
                    if let Some(value) = field_value.as_text() {
                        row.push(value);
                    }
                }
            }
            wtr.write_record(row)?;
        }
    }
    Ok(wtr.flush()?)
}