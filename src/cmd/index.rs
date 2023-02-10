use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use csv_index::RandomAccessSimple;
use tantivy::{Index, schema::*, tokenizer::*};

use CliResult;
use config::{Config, Delimiter};
use util;

static USAGE: &'static str = "
Creates an index of the given CSV data, which can make other operations like
slicing, splitting and gathering statistics much faster.

Note that this does not accept CSV data on stdin. You must give a file
path. The index is created at 'path/to/input.csv.idx'. The index will be
automatically used by commands that can benefit from it. If the original CSV
data changes after the index is made, commands that try to use it will result
in an error (you have to regenerate the index before it can be used again).

Usage:
    xsv index [options] <input>
    xsv index --help

index options:
    -o, --output <file>    Write index to <file> instead of <input>.idx.
                           Generally, this is not currently useful because
                           the only way to use an index is if it is specially
                           named <input>.idx.
    --fullsearch           When set, will build the index for `xsv fullsearch`.
    --lang <arg>           Only useful with `--fullsearch`. <arg> is a language
                           that can be choosen among arabic, danish, dutch, english,
                           finnish, french, german, greek, hungarian, italian,
                           norwegian, portuguese, romanian, russian, spanish, swedish,
                           tamil, turkish.

Common options:
    -h, --help             Display this message
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character. (default: ,)
";

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
    arg_input: String,
    flag_fullsearch: bool,
    flag_lang: Option<String>,
    flag_output: Option<String>,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    let rconfig = Config::new(&Some(args.arg_input.clone()))
                         .delimiter(args.flag_delimiter);
    let mut rdr = rconfig.reader_file()?;

    if args.flag_fullsearch {
        let lang = match args.flag_lang {
            None => "english".to_string(),
            Some(lang) => lang,
        };
        if !LANGUAGES.contains(&&lang[..]) {
            return fail!(format!("Unknown \"{}\" language found", lang));
        }

        let pidx = match args.flag_output {
            None => util::idx_fullsearch_path(&Path::new(&args.arg_input), &lang),
            Some(p) => PathBuf::from(&p),
        };
        if pidx.exists() {
            fs::remove_dir_all(pidx.clone()).unwrap();
        }
        fs::create_dir_all(pidx.as_path()).unwrap();

        let lang_stemmer = match &lang[..] {
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

        let mut schema_builder = Schema::builder();
        let text_field_indexing = TextFieldIndexing::default()
            .set_tokenizer("custom")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);
        let text_options = TextOptions::default()
            .set_indexing_options(text_field_indexing)
            .set_stored();
        let mut fields: Vec::<Field> = Vec::new();
        let headers = rdr.byte_headers()?.clone();
        for (i, _) in headers.into_iter().enumerate() {
            fields.push(schema_builder.add_text_field(&i.to_string(), text_options.clone()));
        }
        let schema = schema_builder.build();
        let index = Index::create_in_dir(&pidx, schema.clone())?;
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
    } else {
        let pidx = match args.flag_output {
            None => util::idx_path(&Path::new(&args.arg_input)),
            Some(p) => PathBuf::from(&p),
        };
        let mut wtr = io::BufWriter::new(fs::File::create(&pidx)?);
        RandomAccessSimple::create(&mut rdr, &mut wtr)?;
    }
    Ok(())
}
