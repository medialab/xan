use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::PathBuf;
use std::str::from_utf8;

use bstr::ByteSlice;
use colored::Colorize;
use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use pariter::IteratorExt;
use regex::bytes::Regex;
use scraper::{Html, Selector};
use serde_json::{Map, Value};
use url::Url;

use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicValue, ScrapingProgram};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::{CliError, CliResult};

// IO helpers
fn open(input_dir: &str, filename: &str) -> io::Result<Box<dyn Read>> {
    let mut path = PathBuf::from(input_dir);
    path.push(filename);

    let file = File::open(path)?;

    Ok(if filename.ends_with(".gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    })
}

fn read_string(input_dir: &str, filename: &str) -> io::Result<String> {
    let mut string = String::new();

    open(input_dir, filename)?.read_to_string(&mut string)?;

    Ok(string)
}

fn read_bytes(input_dir: &str, filename: &str) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();

    open(input_dir, filename)?.read_to_end(&mut bytes)?;

    Ok(bytes)
}

const PREBUFFER_SIZE: usize = 4096;

fn find_head(bytes: &[u8]) -> Option<usize> {
    bytes.rfind(b"</head>").or_else(|| bytes.rfind(b"</HEAD>"))
}

fn read_up_to_head(input_dir: &str, filename: &str) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; PREBUFFER_SIZE];

    let mut reader = open(input_dir, filename)?;

    while let Ok(len) = reader.read(&mut buffer) {
        if len == 0 {
            break;
        }

        bytes.extend_from_slice(&buffer[..len]);

        let offset = bytes.len().saturating_sub(len + 7);
        let haystack = &bytes[offset..];

        debug_assert!(haystack.len() <= PREBUFFER_SIZE + 7);

        if let Some(i) = find_head(haystack) {
            bytes.truncate(offset + i + 7);
            break;
        }
    }

    Ok(bytes)
}

fn read_html(input_dir: &str, filename: &str) -> io::Result<Html> {
    read_string(input_dir, filename).map(|string| Html::parse_document(&string))
}

#[derive(Debug)]
enum ScraperTarget<'a> {
    HtmlCell(&'a [u8]),
    HtmlFile(&'a str, &'a str),
}

fn guard_invalid_html_cell(cell: &[u8]) -> CliResult<()> {
    if !cell.is_empty() && !looks_like_html(cell) {
        Err(format!(
            "encountered cell value that does not look like HTML: {}!\nDid you forget to give {}?",
            from_utf8(cell).unwrap().green(),
            "-I/--input-dir".cyan()
        ))?
    } else {
        Ok(())
    }
}

impl ScraperTarget<'_> {
    fn prebuffer_up_to_head(&self) -> CliResult<Cow<[u8]>> {
        match self {
            Self::HtmlCell(cell) => {
                guard_invalid_html_cell(cell)?;

                Ok(match find_head(cell) {
                    Some(i) => Cow::Borrowed(&cell[..i + 7]),
                    None => Cow::Borrowed(cell),
                })
            }
            Self::HtmlFile(input_dir, filename) => {
                Ok(Cow::Owned(read_up_to_head(input_dir, filename)?))
            }
        }
    }

    fn read_bytes(&self) -> CliResult<Cow<[u8]>> {
        match self {
            Self::HtmlCell(cell) => {
                guard_invalid_html_cell(cell)?;

                Ok(Cow::Borrowed(cell))
            }
            Self::HtmlFile(input_dir, filename) => Ok(Cow::Owned(read_bytes(input_dir, filename)?)),
        }
    }

    fn read_html(&self) -> CliResult<Html> {
        match self {
            Self::HtmlCell(cell) => {
                guard_invalid_html_cell(cell)?;

                Ok(Html::parse_document(
                    from_utf8(cell).expect("invalid utf-8"),
                ))
            }
            Self::HtmlFile(input_dir, filename) => Ok(read_html(input_dir, filename)?),
        }
    }
}

// Scraper abstractions
lazy_static! {
    // Regexes
    static ref HTML_LIKE_REGEX: Regex =
        Regex::new(r"^\s*<(?:html|head|body|title|meta|link|span|div|img|ul|ol|[ap!?])").unwrap();
    static ref SCRIPT_REGEX: Regex = Regex::new(r"<script[^>]*>.*?</script>").unwrap();
    static ref URLS_IN_HTML_REGEX: Regex =
        Regex::new(r#"<a[^>]*\shref=(?:"([^"]*)"|'([^']*)'|([^\s>]*))[^>]*>"#).unwrap();

    // Selectors
    static ref HEAD_SELECTOR: Selector = Selector::parse("head").unwrap();
    static ref TITLE_SELECTOR: Selector = Selector::parse("title").unwrap();
    static ref CANONICAL_SELECTOR: Selector =
        Selector::parse("link[rel=canonical]").unwrap();
    static ref JSON_LD_SELECTOR: Selector = Selector::parse("script[type=\"application/ld+json\"]").unwrap();
}

fn looks_like_html(bytes: &[u8]) -> bool {
    HTML_LIKE_REGEX.is_match(bytes)
}

#[derive(Clone, Debug)]
struct CustomScraper {
    program: ScrapingProgram,
    foreach: Option<Selector>,
}

impl CustomScraper {
    fn is_plural(&self) -> bool {
        self.foreach.is_some()
    }

    fn run_singular(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        html: &Html,
    ) -> CliResult<Vec<DynamicValue>> {
        Ok(self.program.run_singular(index, record, html)?)
    }

    fn scrape(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        target: &ScraperTarget,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        let html = target.read_html()?;

        if let Some(selector) = &self.foreach {
            Ok(self
                .program
                .run_plural(index, record, &html, selector)
                .collect::<Result<Vec<_>, _>>()?)
        } else {
            Ok(vec![self.program.run_singular(index, record, &html)?])
        }
    }
}

#[derive(Clone, Debug)]
enum Scraper {
    Head,
    Urls(Option<usize>),
    Article(Option<CustomScraper>),
    Custom(CustomScraper),
}

impl Scraper {
    fn is_plural(&self) -> bool {
        match self {
            Self::Head | Self::Article(_) => false,
            Self::Urls(_) => true,
            Self::Custom(inner) => inner.is_plural(),
        }
    }

    fn names(&self) -> Vec<String> {
        match self {
            Self::Head => vec!["title".to_string(), "canonical_url".to_string()],
            Self::Urls(_) => vec!["url".to_string()],
            Self::Article(scraper_opt) => {
                let mut names = vec![
                    "canonical_url".to_string(),
                    "headline".to_string(),
                    "description".to_string(),
                    "date_created".to_string(),
                    "date_published".to_string(),
                    "date_modified".to_string(),
                    "section".to_string(),
                    "keywords".to_string(),
                    "authors".to_string(),
                    "image".to_string(),
                    "image_caption".to_string(),
                    "free".to_string(),
                ];

                if let Some(scraper) = scraper_opt {
                    for name in scraper.program.names() {
                        names.push(name.to_string());
                    }
                }

                names
            }
            Self::Custom(scraper) => scraper
                .program
                .names()
                .map(|name| name.to_string())
                .collect(),
        }
    }

    fn scrape_head(&self, target: &ScraperTarget) -> CliResult<Vec<DynamicValue>> {
        let bytes = target.prebuffer_up_to_head()?;
        let html = Html::parse_document(from_utf8(&bytes).unwrap());

        Ok(html
            .select(&HEAD_SELECTOR)
            .next()
            .map(|head_element| {
                vec![
                    // title
                    DynamicValue::from(
                        head_element
                            .select(&TITLE_SELECTOR)
                            .next()
                            .map(|element| element.text().collect::<String>().trim().to_string()),
                    ),
                    // canonical_url
                    DynamicValue::from(head_element.select(&CANONICAL_SELECTOR).next().and_then(
                        |element| element.attr("href").map(|href| href.trim().to_string()),
                    )),
                ]
            })
            .unwrap_or_else(|| vec![DynamicValue::None; 2]))
    }

    fn scrape_urls(
        &self,
        record: &csv::ByteRecord,
        target: &ScraperTarget,
        url_column_index: Option<usize>,
    ) -> CliResult<Vec<DynamicValue>> {
        let bytes = target.read_bytes()?;
        let bytes = SCRIPT_REGEX.replace_all(&bytes, b"");

        let mut urls = Vec::new();

        let base_url_opt =
            url_column_index.and_then(|i| Url::parse(from_utf8(&record[i]).unwrap()).ok());

        for caps in URLS_IN_HTML_REGEX.captures_iter(&bytes) {
            let url = if let Some(m) = caps.get(1) {
                m.as_bytes().trim_end_with(|c| c == '"')
            } else if let Some(m) = caps.get(2) {
                m.as_bytes().trim_end_with(|c| c == '\'')
            } else {
                &caps[3]
            };

            if let Some(base_url) = &base_url_opt {
                if let Ok(joined_url) = base_url.join(from_utf8(url).unwrap()) {
                    // TODO: canonicalize
                    urls.push(DynamicValue::from(joined_url.to_string()));
                }
            } else {
                urls.push(DynamicValue::from(url));
            }
        }

        Ok(urls)
    }

    fn scrape_article(&self, html: &Html) -> CliResult<Vec<DynamicValue>> {
        let mut output = vec![DynamicValue::None; 12];

        if let Some(head_element) = html.select(&HEAD_SELECTOR).next() {
            if let Some(canonical_element) = head_element.select(&CANONICAL_SELECTOR).next() {
                if let Some(link) = canonical_element.attr("href") {
                    output[0] = DynamicValue::from(link);
                }
            }
        }

        let mut json_ld: Option<Map<String, Value>> = None;

        'main: for script_element in html.select(&JSON_LD_SELECTOR) {
            if let Ok(value) = serde_json::from_str(&script_element.text().collect::<String>()) {
                // Single map variant
                if let Value::Object(map) = value {
                    if let Some(v) = map.get("@type") {
                        if let Some(t) = v.as_str() {
                            let t = t.to_lowercase();

                            if t == "http://schema.org/newsarticle" || t == "newsarticle" {
                                json_ld = Some(map);
                                break 'main;
                            }
                        }
                    }
                }
                // Multiple map variants
                else if let Value::Array(list) = value {
                    for item in list {
                        if let Value::Object(map) = item {
                            if let Some(v) = map.get("@type") {
                                if let Some(t) = v.as_str() {
                                    let t = t.to_lowercase();

                                    if t == "http://schema.org/newsarticle" || t == "newsarticle" {
                                        json_ld = Some(map);
                                        break 'main;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        fn clean_htmlish(string: &str) -> DynamicValue {
            DynamicValue::from(html_escape::decode_html_entities(string).trim())
        }

        // dbg!(&json_ld);

        if let Some(data) = json_ld {
            macro_rules! extract_raw {
                ($index: expr, $key: expr) => {
                    if let Some(d) = data.get($key) {
                        if let Some(text) = d.as_str() {
                            output[$index] = DynamicValue::from(text);
                        }
                    }
                };
            }

            macro_rules! extract_text {
                ($index: expr, $key: expr) => {
                    if let Some(d) = data.get($key) {
                        if let Some(text) = d.as_str() {
                            output[$index] = clean_htmlish(text);
                        }
                    }
                };
            }

            macro_rules! extract_plural {
                ($index: expr, $key: expr) => {
                    if let Some(d) = data.get($key) {
                        if let Some(l) = d.as_array() {
                            output[$index] = l
                                .iter()
                                .filter_map(|d| d.as_str())
                                .collect::<Vec<_>>()
                                .join("§")
                                .into();
                        } else if let Some(t) = d.as_str() {
                            output[$index] = t.into();
                        }
                    }
                };
            }

            extract_text!(1, "headline");
            extract_text!(2, "description");
            extract_raw!(3, "dateCreated");
            extract_raw!(4, "datePublished");
            extract_raw!(5, "dateModified");
            extract_plural!(6, "articleSection");
            extract_plural!(7, "keywords");

            if let Some(author) = data.get("author") {
                if let Some(name) = author.as_str() {
                    output[8] = name.into();
                } else if let Some(names) = author.as_array() {
                    output[8] = names
                        .iter()
                        .filter_map(|author| {
                            if let Some(text) = author.as_str() {
                                return Some(text);
                            } else if let Some(author_data) = author.as_object() {
                                if let Some(name) = author_data.get("name") {
                                    if let Some(text) = name.as_str() {
                                        return Some(text);
                                    }
                                }
                            }

                            None
                        })
                        .collect::<Vec<_>>()
                        .join("§")
                        .into();
                }

                if let Some(image) = data.get("image") {
                    if let Some(url) = image.as_str() {
                        output[9] = url.into();
                    } else if let Some(image_data) = image.as_object() {
                        if let Some(url) = image_data.get("url") {
                            if let Some(text) = url.as_str() {
                                output[9] = text.into();
                            }
                        }

                        if let Some(caption) =
                            image_data.get("caption").or_else(|| image_data.get("name"))
                        {
                            if let Some(text) = caption.as_str() {
                                output[10] = text.into();
                            }
                        }
                    }
                }

                output[11] = DynamicValue::from(false);

                if let Some(free) = data.get("isAccessibleForFree") {
                    if let Some(v) = free.as_bool() {
                        output[11] = DynamicValue::from(v);
                    }
                }
            }
        }

        Ok(output)
    }

    fn scrape(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        target: &ScraperTarget,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        match self {
            Self::Head => Ok(vec![self.scrape_head(&target)?]),
            Self::Urls(url_column_index) => {
                let urls = self.scrape_urls(record, &target, *url_column_index)?;

                Ok(urls.into_iter().map(|url| vec![url]).collect())
            }
            Self::Article(scraper_opt) => {
                let html = target.read_html()?;

                let mut output = self.scrape_article(&html)?;

                if let Some(scraper) = scraper_opt {
                    let supplementary_output = scraper.run_singular(index, record, &html)?;

                    output.extend(supplementary_output);
                }

                Ok(vec![output])
            }
            Self::Custom(scraper) => scraper.scrape(index, record, &target),
        }
    }

    fn scrape_or_report(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        target: &ScraperTarget,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        self.scrape(index, record, target).map_err(|err| {
            CliError::Other(format!(
                "Row index {}{}\n{}",
                index,
                match target {
                    ScraperTarget::HtmlFile(_, path) => {
                        format!(", in path {}", path.cyan())
                    }
                    _ => "".to_string(),
                },
                err
            ))
        })
    }
}

static USAGE: &str = "
Scrape HTML files to output tabular CSV data.

This command can either process a CSV file with a column containing
raw HTML, or a CSV file with a column of paths to read, relative to what is given
to the -I/--input-dir flag.

Scraping a HTML column:

    $ xan scrape head document docs.csv > enriched-docs.csv

Scraping HTML files on disk, using the -I/--input-dir flag:

    $ xan scrape head path -I ./downloaded docs.csv > enriched-docs.csv

Then, this command knows how to scrape typical stuff from HTML such
as titles, urls and other metadata using very optimized routines
or can let you define a custom scraper that you can give through
the -e/--evaluate or -f/--evaluate-file.

The command can of course use multiple CPUs to go faster using -p/--parallel
or -t/--threads.

# Builtin scrapers

Here is the list of `xan scrape` builtin scrapers along with the columns they
will add to the output:

\"head\": will scrape typical metadata found in <head> tags. Outputs one row
per input row with following columns:
    - title
    - canonical_url

\"urls\": will scrape all urls found in <a> tags in the document. Outputs one
row per scraped url per input row with following columns:
    - url

\"article\": will scrape typical news article metadata by analyzing the <head>
tag and JSON-LD data (note that you can combine this one with the -e/-f flags
to add custom data to the output, e.g. to scrape the article text). Outputs one
row per input row with the following columns:
    - canonical_url
    - headline
    - description
    - date_created
    - date_published
    - date_modified
    - section
    - keywords
    - authors
    - image
    - image_caption
    - free

# Custom scrapers

When using -e/--evaluate or -f/--evaluate-file, this command is able to
leverage a custom CSS-like language to describe exactly what you want to
scrape.

Given scraper will either run once per HTML document or one time per
element matching the CSS selector given to -F/--foreach.

Example scraping the first h2 title from each document:

    $ xan scrape -e 'h2 > a {title: text; url: attr(\"href\");}' html docs.csv

Example scraping all the h2 title from each document:

    $ xan scrape --foreach 'h2 > a' -e '& {title: text; url: attr(\"href\");}' html docs.csv

A full reference of this language can be found using `xan help scraping`.

# How many output rows per input row?

Scrapers can either output exactly one row per input row or 0 to n output rows
per input row.

Scrapers outputting exactly one row per input row: \"head\", \"article\", any
scraper given to -e/-f WITHOUT -F/--foreach.

Scrapers outputting 0 to n rows per input row: \"urls\", any scraper given to -e/-f
WITH -F/--foreach.

It can be useful sometimes to use the -k/--keep flag to select the input columns
to keep in the output. Note that using this flag with an empty selection (-k '')
means outputting only the scraped columns.

Usage:
    xan scrape head <column> [options] [<input>]
    xan scrape urls <column> [options] [<input>]
    xan scrape article <column> [options] [<input>]
    xan scrape -e <expr> <column> [options] [<input>]
    xan scrape -f <path> <column> [options] [<input>]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>       If given, evaluate the given scraping expression.
    -f, --evaluate-file <path>  If given, evaluate the scraping expression found
                                in file at <path>.
    -I, --input-dir <path>      If given, target column will be understood
                                as relative path to read from this input
                                directory instead.
    -k, --keep <column>         Selection of columns from the input to keep in
                                the output. Default is to keep all columns from input.
    -p, --parallel              Whether to use parallelization to speed up computations.
                                Will automatically select a suitable number of threads to use
                                based on your number of cores. Use -t, --threads if you want to
                                indicate the number of threads yourself.
    -t, --threads <threads>     Parellize computations using this many threads. Use -p, --parallel
                                if you want the number of threads to be automatically chosen instead.

scrape url/links options:
    -u, --url-column <column>  Column containing the base url for given HTML.

scrape -e/--evaluate & -f/--evaluate-file options:
    -F, --foreach <css>  If given, will return one row per element matching
                         the CSS selector in target document, instead of returning
                         a single row per document.
    --sep <char>            Separator to use when serializing lists.
                         [default: |]

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
    arg_input: Option<String>,
    arg_column: SelectColumns,
    cmd_head: bool,
    cmd_urls: bool,
    cmd_article: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_evaluate: Option<String>,
    flag_evaluate_file: Option<String>,
    flag_foreach: Option<String>,
    flag_url_column: Option<SelectColumns>,
    flag_input_dir: Option<String>,
    flag_keep: Option<SelectColumns>,
    flag_sep: String,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

impl Args {
    fn resolve(&mut self) -> CliResult<()> {
        if self.flag_evaluate.is_some() && self.flag_evaluate_file.is_some() {
            Err("cannot use both -e/--evaluate & -f/--evaluate-file!")?;
        }

        if let Some(path) = &self.flag_evaluate_file {
            self.flag_evaluate = Some(fs::read_to_string(path)?);
        }

        Ok(())
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = util::get_args(USAGE, argv)?;
    args.resolve()?;

    let conf = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .select(args.arg_column)
        .no_headers(args.flag_no_headers);

    let mut reader = conf.reader()?;
    let headers = reader.byte_headers()?.clone();
    let column_index = conf.single_selection(&headers)?;

    let parallelization = match (args.flag_parallel, args.flag_threads) {
        (true, None) => Some(None),
        (_, Some(count)) => Some(Some(count)),
        _ => None,
    };

    if args.flag_evaluate.is_none() && args.flag_foreach.is_some() {
        Err("--foreach only works with -e/--evaluate!")?;
    }

    let url_column_index = args
        .flag_url_column
        .as_ref()
        .map(|s| s.single_selection(&headers, !args.flag_no_headers))
        .transpose()?;

    let scraper = if args.cmd_head {
        Scraper::Head
    } else if args.cmd_article {
        if args.flag_foreach.is_some() {
            Err("-F/--foreach does not work with `xan scrape article -e`!")?;
        }

        Scraper::Article(
            args.flag_evaluate
                .as_ref()
                .map(|code| -> CliResult<CustomScraper> {
                    Ok(CustomScraper {
                        program: ScrapingProgram::parse(code, &headers)?,
                        foreach: None,
                    })
                })
                .transpose()?,
        )
    } else if args.cmd_urls {
        Scraper::Urls(url_column_index)
    } else {
        Scraper::Custom(CustomScraper {
            program: ScrapingProgram::parse(args.flag_evaluate.as_ref().unwrap(), &headers)?,
            foreach: args
                .flag_foreach
                .as_ref()
                .map(|css| {
                    Selector::parse(css).map_err(|_| format!("invalid CSS selector: {}", css))
                })
                .transpose()?,
        })
    };

    let keep = args
        .flag_keep
        .map(|s| {
            if s.is_empty() {
                Ok(Selection::empty())
            } else {
                s.selection(&headers, !args.flag_no_headers)
            }
        })
        .transpose()?;

    let mut writer = Config::new(&args.flag_output).writer()?;

    let scraper_field_names = scraper.names();
    let padding = scraper_field_names.len();

    if !args.flag_no_headers {
        let mut output_headers = headers.clone();

        if let Some(keep_sel) = &keep {
            output_headers = keep_sel.select(&output_headers).collect();
        }

        for name in scraper_field_names {
            output_headers.push_field(name.as_bytes());
        }

        writer.write_byte_record(&output_headers)?;
    }

    if let Some(threads) = parallelization {
        reader
            .into_byte_records()
            .enumerate()
            .parallel_map_custom(
                |o| {
                    if let Some(count) = threads {
                        o.threads(count)
                    } else {
                        o
                    }
                },
                move |(index, result)| -> CliResult<(csv::ByteRecord, Vec<Vec<DynamicValue>>)> {
                    let record = result?;

                    let cell = &record[column_index];

                    if cell.trim().is_empty() {
                        return Ok((
                            record,
                            if scraper.is_plural() {
                                vec![]
                            } else {
                                vec![vec![DynamicValue::None; padding]]
                            },
                        ));
                    }

                    let target = if let Some(input_dir) = &args.flag_input_dir {
                        ScraperTarget::HtmlFile(input_dir, from_utf8(cell).expect("invalid utf-8"))
                    } else {
                        ScraperTarget::HtmlCell(cell)
                    };

                    let output_rows = scraper.scrape_or_report(index, &record, &target)?;

                    Ok((record, output_rows))
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (record, output_rows) = result?;

                for output_row in output_rows {
                    let mut output_record = if let Some(keep_sel) = &keep {
                        keep_sel.select(&record).collect()
                    } else {
                        record.clone()
                    };

                    for value in output_row {
                        output_record.push_field(
                            &value.serialize_as_bytes_with_options(args.flag_sep.as_bytes()),
                        );
                    }

                    writer.write_byte_record(&output_record)?;
                }

                Ok(())
            })?;
    } else {
        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while reader.read_byte_record(&mut record)? {
            let cell = &record[column_index];

            let output_rows = {
                if cell.trim().is_empty() {
                    if scraper.is_plural() {
                        vec![]
                    } else {
                        vec![vec![DynamicValue::None; padding]]
                    }
                } else {
                    let target = if let Some(input_dir) = &args.flag_input_dir {
                        ScraperTarget::HtmlFile(input_dir, from_utf8(cell).expect("invalid utf-8"))
                    } else {
                        ScraperTarget::HtmlCell(cell)
                    };

                    scraper.scrape_or_report(index, &record, &target)?
                }
            };

            for output_row in output_rows {
                let mut output_record = if let Some(keep_sel) = &keep {
                    keep_sel.select(&record).collect()
                } else {
                    record.clone()
                };

                for value in output_row {
                    output_record.push_field(
                        &value.serialize_as_bytes_with_options(args.flag_sep.as_bytes()),
                    );
                }

                writer.write_byte_record(&output_record)?;
            }

            index += 1;
        }
    }

    Ok(writer.flush()?)
}
