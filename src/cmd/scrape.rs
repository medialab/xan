use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read};
use std::iter::once;
use std::path::PathBuf;
use std::str::from_utf8;

use bstr::ByteSlice;
use colored::Colorize;
use encoding::{DecoderTrap, EncodingRef, label::encoding_from_whatwg_label};
use flate2::read::MultiGzDecoder;
use lazy_static::lazy_static;
use pariter::IteratorExt;
use regex::bytes::Regex;
use scraper::{Html, Selector};
use serde_json::{Map, Value};
use simd_csv::ByteRecord;
use url::Url;

use crate::config::{Config, Delimiter};
use crate::moonblade::{AmortizedSerializer, DynamicValue, ScrapingProgram};
use crate::select::{SelectedColumns, Selection};
use crate::urls::should_follow_href;
use crate::utils;
use crate::{CliError, CliResult};

// IO helpers
fn open(input_dir: &str, filename: &str) -> io::Result<Box<dyn Read>> {
    let mut path = PathBuf::from(input_dir);
    path.push(filename);

    let file = File::open(path)?;

    Ok(if filename.ends_with(".gz") {
        Box::new(MultiGzDecoder::new(file))
    } else {
        Box::new(file)
    })
}

fn read_string(
    input_dir: &str,
    filename: &str,
    encoding: Option<EncodingRef>,
) -> io::Result<String> {
    if let Some(encoding_ref) = encoding {
        let bytes = read_bytes(input_dir, filename)?;

        Ok(encoding_ref
            .decode(&bytes, DecoderTrap::Replace)
            .expect("could not decode"))
    } else {
        let mut string = String::new();

        open(input_dir, filename)?.read_to_string(&mut string)?;

        Ok(string)
    }
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

fn read_html(input_dir: &str, filename: &str, encoding: Option<EncodingRef>) -> io::Result<Html> {
    read_string(input_dir, filename, encoding).map(|string| Html::parse_document(&string))
}

enum ScraperTarget<'a> {
    HtmlCell(&'a [u8]),
    HtmlFile {
        input_dir: &'a str,
        filename: &'a str,
        encoding: Option<EncodingRef>,
    },
}

impl ScraperTarget<'_> {
    fn decode<'b>(&self, bytes: &'b [u8]) -> Cow<'b, str> {
        match self {
            Self::HtmlCell(_) => Cow::Borrowed(from_utf8(bytes).expect("could not decode")),
            Self::HtmlFile { encoding, .. } => {
                if let Some(encoding_ref) = encoding {
                    Cow::Owned(
                        encoding_ref
                            .decode(bytes, DecoderTrap::Replace)
                            .expect("could not decode"),
                    )
                } else {
                    Cow::Borrowed(from_utf8(bytes).expect("could not decode"))
                }
            }
        }
    }
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
    fn prebuffer_up_to_head(&self) -> CliResult<Cow<'_, [u8]>> {
        match self {
            Self::HtmlCell(cell) => {
                guard_invalid_html_cell(cell)?;

                Ok(match find_head(cell) {
                    Some(i) => Cow::Borrowed(&cell[..i + 7]),
                    None => Cow::Borrowed(cell),
                })
            }
            Self::HtmlFile {
                input_dir,
                filename,
                ..
            } => Ok(Cow::Owned(read_up_to_head(input_dir, filename)?)),
        }
    }

    fn read_bytes(&self) -> CliResult<Cow<'_, [u8]>> {
        match self {
            Self::HtmlCell(cell) => {
                guard_invalid_html_cell(cell)?;

                Ok(Cow::Borrowed(cell))
            }
            Self::HtmlFile {
                input_dir,
                filename,
                ..
            } => Ok(Cow::Owned(read_bytes(input_dir, filename)?)),
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
            Self::HtmlFile {
                input_dir,
                filename,
                encoding,
            } => Ok(read_html(input_dir, filename, *encoding)?),
        }
    }
}

#[derive(Clone, Copy)]
enum InputMode {
    Path,
    Document(usize),
}

enum InputPayload {
    Path(String, ByteRecord),
    Document(usize, ByteRecord),
}

#[derive(Default)]
struct ScraperContext {
    url: Option<Url>,
}

impl ScraperContext {
    fn new(url: Option<&str>) -> Result<Self, url::ParseError> {
        Ok(Self {
            url: url.map(Url::parse).transpose()?,
        })
    }
}

struct Input {
    payload: InputPayload,
    encoding: Option<EncodingRef>,
    context: ScraperContext,
}

impl Input {
    fn is_empty(&self) -> bool {
        match &self.payload {
            InputPayload::Path(path, _) => path.trim().is_empty(),
            InputPayload::Document(i, record) => record[*i].trim().is_empty(),
        }
    }

    fn as_target<'a>(&'a self, input_dir: &'a str) -> ScraperTarget<'a> {
        match &self.payload {
            InputPayload::Path(path, _) => ScraperTarget::HtmlFile {
                input_dir,
                filename: path,
                encoding: self.encoding,
            },
            InputPayload::Document(i, record) => ScraperTarget::HtmlCell(&record[*i]),
        }
    }

    fn as_record(&self) -> &ByteRecord {
        match &self.payload {
            InputPayload::Path(_, record) | InputPayload::Document(_, record) => record,
        }
    }
}

impl From<InputPayload> for Input {
    #[inline]
    fn from(payload: InputPayload) -> Self {
        Self {
            payload,
            encoding: None,
            context: Default::default(),
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
    static ref IMG_SELECTOR: Selector = Selector::parse("img[src]").unwrap();
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
    fn padding_len(&self) -> usize {
        self.program.capacity
    }

    fn is_plural(&self) -> bool {
        self.foreach.is_some()
    }

    fn run_singular(
        &self,
        index: usize,
        record: &ByteRecord,
        html: &Html,
    ) -> CliResult<Vec<DynamicValue>> {
        Ok(self.program.run_singular(index, record, html)?)
    }

    fn scrape(
        &self,
        index: usize,
        record: &ByteRecord,
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

// TODO: Urls & Images could take a future base url if required
#[derive(Clone, Debug)]
enum Scraper {
    Head,
    Urls,
    Images,
    Article(Option<CustomScraper>),
    Custom(CustomScraper),
}

impl Scraper {
    #[inline]
    fn is_plural(&self) -> bool {
        match self {
            Self::Head | Self::Article(_) => false,
            Self::Urls | Self::Images => true,
            Self::Custom(inner) => inner.is_plural(),
        }
    }

    #[inline]
    fn padding_len(&self) -> usize {
        match self {
            Self::Urls | Self::Images => 1,
            Self::Head => 2,
            Self::Article(scraper_opt) => {
                scraper_opt
                    .as_ref()
                    .map(|scraper| scraper.padding_len())
                    .unwrap_or(0)
                    + 12
            }
            Self::Custom(scraper) => scraper.padding_len(),
        }
    }

    fn names(&self) -> Vec<&'_ str> {
        match self {
            Self::Head => vec!["title", "canonical_url"],
            Self::Urls => vec!["url"],
            Self::Images => vec!["src"],
            Self::Article(scraper_opt) => {
                let mut names = vec![
                    "canonical_url",
                    "headline",
                    "description",
                    "date_created",
                    "date_published",
                    "date_modified",
                    "section",
                    "keywords",
                    "authors",
                    "image",
                    "image_caption",
                    "free",
                ];

                if let Some(scraper) = scraper_opt {
                    for name in scraper.program.names() {
                        names.push(name);
                    }
                }

                names
            }
            Self::Custom(scraper) => scraper.program.names().collect(),
        }
    }

    fn scrape_head(&self, target: &ScraperTarget) -> CliResult<Vec<DynamicValue>> {
        let bytes = target.prebuffer_up_to_head()?;
        let html = Html::parse_document(&target.decode(&bytes));

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
        target: &ScraperTarget,
        context: &ScraperContext,
    ) -> CliResult<Vec<DynamicValue>> {
        let bytes = target.read_bytes()?;
        let bytes = SCRIPT_REGEX.replace_all(&bytes, b"");

        let mut urls = Vec::new();

        for caps in URLS_IN_HTML_REGEX.captures_iter(&bytes) {
            let url = if let Some(m) = caps.get(1) {
                m.as_bytes().trim_end_with(|c| c == '"')
            } else if let Some(m) = caps.get(2) {
                m.as_bytes().trim_end_with(|c| c == '\'')
            } else {
                &caps[3]
            };

            if let Some(base_url) = &context.url {
                if let Ok(joined_url) = base_url.join(&target.decode(&bytes)) {
                    // TODO: canonicalize
                    urls.push(DynamicValue::from(joined_url.to_string()));
                }
            } else {
                urls.push(DynamicValue::from(url));
            }
        }

        Ok(urls)
    }

    fn scrape_images(
        &self,
        target: &ScraperTarget,
        context: &ScraperContext,
    ) -> CliResult<Vec<String>> {
        let html = target.read_html()?;
        let mut images = Vec::new();

        for img_element in html.select(&IMG_SELECTOR) {
            let src = img_element.attr("src").unwrap(); // Safe to unwrap because of selector guard

            if should_follow_href(src) {
                match &context.url {
                    Some(base_url) => {
                        if let Ok(joined_src) = base_url.join(src) {
                            // TODO: canonicalize
                            images.push(joined_src.to_string());
                        }
                    }
                    None => {
                        images.push(src.to_string());
                    }
                };
            }
        }

        Ok(images)
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

    // TODO: could be lazy iterators sometimes
    fn scrape(
        &self,
        input_dir: &str,
        row_index: usize,
        input: &Input,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        if input.is_empty() {
            return Ok(if self.is_plural() {
                vec![]
            } else {
                vec![vec![DynamicValue::None; self.padding_len()]]
            });
        }

        let target = input.as_target(input_dir);
        let record = input.as_record();
        let context = &input.context;

        match self {
            Self::Head => Ok(vec![self.scrape_head(&target)?]),
            Self::Urls => {
                let urls = self.scrape_urls(&target, context)?;

                Ok(urls.into_iter().map(|url| vec![url]).collect())
            }
            Self::Images => {
                let images = self.scrape_images(&target, context)?;

                Ok(images
                    .into_iter()
                    .map(|src| vec![DynamicValue::from(src)])
                    .collect())
            }
            Self::Article(scraper_opt) => {
                let html = target.read_html()?;

                let mut output = self.scrape_article(&html)?;

                if let Some(scraper) = scraper_opt {
                    let supplementary_output = scraper.run_singular(row_index, record, &html)?;

                    output.extend(supplementary_output);
                }

                Ok(vec![output])
            }
            Self::Custom(scraper) => scraper.scrape(row_index, record, &target),
        }
    }

    fn scrape_or_report(
        &self,
        input_dir: &str,
        row_index: usize,
        input: &Input,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        self.scrape(input_dir, row_index, input).map_err(|err| {
            CliError::Other(format!(
                "Row index {}{}\n{}",
                row_index,
                match &input.payload {
                    InputPayload::Path(filename, _) => {
                        format!(", in path {}", filename.cyan())
                    }
                    _ => "".to_string(),
                },
                err
            ))
        })
    }
}

static USAGE: &str = "
Scrape HTML files (or any kind of XML files, really) and return structured
tabular data from the result.

This command can process a variety of different sources:

Paths to HTML documents on disk, by default
    $ xan scrape head page1.html page2.html page3.html > result.csv
    $ xan scrape head **/*.html > result.csv

Paths to HTML documents collected using a glob pattern, using --glob
    $ xan scrape head --glob '*.csv' > result.csv

A text file containing a HTML document path per line, using --paths
    $ xan scrape head --paths paths.txt > result.csv

A CSV file containing a column with HTML document paths, using --paths & --path-column
    $ xan scrape head --paths path.csv --path-column path > result.csv

A CSV file containing a column of inline HTML documents, using --docs & --doc-column
    $ xan scrape head --docs documents.csv --doc-column html > result.csv

A single HTML document fed through stdin, using -D/--stdin-doc
    $ curl -L https://www.lemonde.fr/ | xan scrape head -D > result.csv

Now regarding what we can scrape, th command knows how to extract typical stuff
from HTML documents such as titles, urls and other metadata using very optimized
routines.

Or you remain free to define a custom scraper that you can give through
the -e/--evaluate or -f/--evaluate-file flags.

Know also that this command is able to use multiple CPUs to go faster using -p/--parallel
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

\"images\": will scrape all downloadable image urls found in <img> tags. Outputs
one row per scraped image per input row with following columns:
    - src

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

Example scraping the first h2 title from a HTML document:

    $ xan scrape -e 'h2 > a {title: text; url: attr(\"href\");}' page.html

Example scraping all the h2 titles from a HTML document:

    $ xan scrape --foreach 'h2 > a' -e '& {title: text; url: attr(\"href\");}' page.html

A full reference of this language can be found using `xan help scraping`.

# How many output rows per input row?

Scrapers can either output exactly one row per input row or 0 to n output rows
per input row.

Scrapers outputting exactly one row per input row: \"head\", \"article\", any
scraper given to -e/-f WITHOUT -F/--foreach.

Scrapers outputting 0 to n rows per input row: \"urls\", \"images\", any scraper
given to -e/-f WITH -F/--foreach.

# Columns kept in the output

By default, all columns from input are kept in the output. You can still choose
columns to keep using the -s/--select flag. This also means you can pass the '!'
empty selection to keep nothing from your input.

Note that if you are not providing a CSV input to the command (i.e. when not
using the --paths nor --docs flags), the command will pretend it was fed with
one having columns depending on how the command was fed its paths or documents:

- Passing <inputs>..., one path per line (--paths without --path-column) or --glob:
  a single \"path\" column containing scraped path.

- Using -D/--stdin-doc:
  a single \"doc\" column containing scraped document.

Then, note that since keeping inline docs is usually undesirable, the column
containing it is dropped by default, unless explicitly given to -s/--select
or when using -k/--keep-doc.

Usage:
    xan scrape (head|urls|article|images) [options] [<inputs>...]
    xan scrape -e <expr> [options] [<inputs>...]
    xan scrape -f <path> [options] [<inputs>...]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>       If given, evaluate the given scraping expression.
    -f, --evaluate-file <path>  If given, evaluate the scraping expression found
                                in file at <path>.
    --paths <input>             If given, reads <input> and consider it as containing
                                one document path per line.
    --path-column <name>        If given with --paths, consider <input> as a CSV file
                                instead and read document paths from selected column.
    --docs <input>              If givens, reads <input> and consider it as a CSV
                                file with a column containing inline documents.
                                Requires --doc-column to be given.
    --doc-column <name>         Selects column containing inline documents given
                                through --docs.
    -D, --stdin-doc             When set, the command will read the content of stdin as
                                a single document. This can be useful when piping the
                                result of `curl` or `wget` into the command directly.
    --glob <pattern>            If given, collects document paths to process by applying
                                the given glob pattern.
    -E, --encoding <name>       Encoding of to read on disk. Will default utf-8.
    -s, --select <columns>      Select columns of input to keep in the output.
    -k, --keep-docs             Whether to keep inline documents (when using --docs or -D/--stdin-doc)
                                in the output.
    -I, --input-dir <path>      When set, processed paths will be read relative to the
                                given base <path>.
                                [default: .]
    -p, --parallel              Whether to use parallelization to speed up computations.
                                Will automatically select a suitable number of threads to use
                                based on your number of cores. Use -t, --threads if you want to
                                indicate the number of threads yourself.
    -t, --threads <threads>     Parellize computations using this many threads. Use -p, --parallel
                                if you want the number of threads to be automatically chosen instead.

scrape url, links, images options:
    -u, --url-column <column>  Column containing the base url for given HTML.

scrape -e/--evaluate & -f/--evaluate-file options:
    -F, --foreach <css>  If given, will return one row per element matching
                         the CSS selector in target document, instead of returning
                         a single row per document.

Common options:
    -h, --help             Display this message
    -o, --output <file>    Write output to <file> instead of stdout.
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    cmd_head: bool,
    cmd_urls: bool,
    cmd_article: bool,
    cmd_images: bool,
    arg_inputs: Vec<String>,
    flag_paths: Option<String>,
    flag_path_column: Option<SelectedColumns>,
    flag_docs: Option<String>,
    flag_doc_column: Option<SelectedColumns>,
    flag_stdin_doc: bool,
    flag_glob: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_evaluate: Option<String>,
    flag_evaluate_file: Option<String>,
    flag_foreach: Option<String>,
    flag_encoding: Option<String>,
    flag_url_column: Option<SelectedColumns>,
    flag_input_dir: String,
    flag_select: Option<SelectedColumns>,
    flag_keep_docs: bool,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

type InputsData<'a> = (
    bool,
    ByteRecord,
    InputMode,
    Box<dyn Iterator<Item = CliResult<Input>> + 'a>,
);

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

    fn encoding(&self) -> CliResult<Option<EncodingRef>> {
        Ok(self
            .flag_encoding
            .as_ref()
            .map(|name| {
                encoding_from_whatwg_label(name).ok_or_else(|| format!("unknown {name} encoding!"))
            })
            .transpose()?)
    }

    fn inputs(&self) -> CliResult<InputsData<'_>> {
        if !self.arg_inputs.is_empty() {
            // Positionals
            Ok((
                true,
                ByteRecord::from(["path"]),
                InputMode::Path,
                Box::new(self.arg_inputs.iter().map(|path| {
                    Ok(InputPayload::Path(path.to_string(), ByteRecord::from([path])).into())
                })),
            ))
        } else if let Some(path) = self.flag_paths.as_ref().or(self.flag_docs.as_ref()) {
            let conf = Config::new(&Some(path.to_string()))
                .delimiter(self.flag_delimiter)
                .no_headers(self.flag_no_headers);

            // Lines
            if self.flag_paths.is_some() && self.flag_path_column.is_none() {
                let reader = conf.io_reader()?;
                let line_reader = BufReader::new(reader);

                Ok((
                    true,
                    ByteRecord::from(["path"]),
                    InputMode::Path,
                    Box::new(line_reader.lines().map(|path| -> CliResult<Input> {
                        let path = path?;
                        let dummy_record = ByteRecord::from([&path]);
                        Ok(InputPayload::Path(path, dummy_record).into())
                    })),
                ))
            }
            // CSV input
            else {
                let mut reader = conf.simd_reader()?;
                let headers = reader.byte_headers()?;

                let sel = if self.flag_paths.is_some() {
                    self.flag_path_column.as_ref().unwrap()
                } else if let Some(sel) = &self.flag_doc_column {
                    sel
                } else {
                    Err("--docs requires --doc-column!")?
                };

                let i = sel.single_selection(headers, !conf.no_headers)?;
                let encoding = self.encoding()?;

                let url_column_index_opt = self
                    .flag_url_column
                    .as_ref()
                    .map(|url_sel| url_sel.single_selection(headers, !conf.no_headers))
                    .transpose()?;

                Ok((
                    !conf.no_headers,
                    headers.clone(),
                    if self.flag_docs.is_some() {
                        InputMode::Document(i)
                    } else {
                        InputMode::Path
                    },
                    Box::new(
                        reader
                            .into_byte_records()
                            .map(move |result| -> CliResult<Input> {
                                let record = result?;

                                let url = if let Some(j) = url_column_index_opt {
                                    Some(from_utf8(&record[j])?)
                                } else {
                                    None
                                };

                                let context = ScraperContext::new(url)?;

                                let payload = if self.flag_docs.is_some() {
                                    InputPayload::Document(i, record)
                                } else {
                                    InputPayload::Path(
                                        String::from_utf8(record[i].to_vec())?,
                                        record,
                                    )
                                };

                                Ok(Input {
                                    payload,
                                    encoding,
                                    context,
                                })
                            }),
                    ),
                ))
            }
        } else if let Some(pattern) = &self.flag_glob {
            // Glob pattern
            Ok((
                true,
                ByteRecord::from(["path"]),
                InputMode::Path,
                Box::new(glob::glob(pattern)?.map(|result| match result {
                    Ok(p) => {
                        let p = p.to_string_lossy().into_owned();
                        let dummy_record = ByteRecord::from([&p]);
                        Ok(InputPayload::Path(p, dummy_record).into())
                    }
                    Err(e) => Err(CliError::from(e)),
                })),
            ))
        } else if self.flag_stdin_doc {
            // Stdin doc
            let mut doc = Vec::new();
            io::stdin().read_to_end(&mut doc)?;

            let record = ByteRecord::from_field(doc);

            Ok((
                true,
                ByteRecord::from(["doc"]),
                InputMode::Document(0),
                Box::new(once(Ok(InputPayload::Document(0, record).into()))),
            ))
        } else {
            unreachable!()
        }
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let mut args: Args = utils::get_args(USAGE, argv)?;
    args.resolve()?;

    let input_modes = !args.arg_inputs.is_empty() as u8
        + args.flag_paths.is_some() as u8
        + args.flag_docs.is_some() as u8
        + args.flag_glob.is_some() as u8
        + args.flag_stdin_doc as u8;

    if input_modes == 0 {
        Err("must take at least one <input> or --paths or --docs or --glob or -D/--stdin-doc!")?;
    } else if input_modes > 1 {
        Err(
            "can only take <inputs>... or --paths or --docs or --glob or -D/--stdin-doc but not a combination of those!",
        )?;
    }

    let threads = utils::parallelization(args.flag_parallel, args.flag_threads);

    if args.flag_evaluate.is_none() && args.flag_foreach.is_some() {
        Err("--foreach only works with -e/--evaluate!")?;
    }

    let (has_headers, headers, input_mode, inputs) = args.inputs()?;

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
                        program: ScrapingProgram::parse(code, &headers, !has_headers)?,
                        foreach: None,
                    })
                })
                .transpose()?,
        )
    } else if args.cmd_urls {
        Scraper::Urls
    } else if args.cmd_images {
        Scraper::Images
    } else {
        Scraper::Custom(CustomScraper {
            program: ScrapingProgram::parse(
                args.flag_evaluate.as_ref().unwrap(),
                &headers,
                !has_headers,
            )?,
            foreach: args
                .flag_foreach
                .as_ref()
                .map(|css| Selector::parse(css).map_err(|_| format!("invalid CSS selector: {css}")))
                .transpose()?,
        })
    };

    let mut sel = if let Some(s) = &args.flag_select {
        s.selection(&headers, has_headers)?
    } else {
        Selection::full(headers.len())
    };

    sel.dedup();

    if let InputMode::Document(i) = input_mode {
        if args.flag_keep_docs {
            if !sel.contains(i) {
                sel = sel.iter().copied().chain([i]).collect::<Vec<_>>().into();
            }
        } else if args.flag_select.is_none() {
            sel = sel
                .iter()
                .copied()
                .filter(|j| i != *j)
                .collect::<Vec<_>>()
                .into();
        }
    }

    let mut writer = Config::new(&args.flag_output).simd_writer()?;
    let mut serializer = AmortizedSerializer::new();

    if has_headers {
        writer.write_record(
            sel.select(&headers)
                .chain(scraper.names().iter().map(|name| name.as_bytes())),
        )?;
    }

    let input_dir = args.flag_input_dir.clone();

    // TODO: amortize allocations, fast path if sel is empty?
    let mut write_output =
        |output_rows: &[Vec<DynamicValue>], record: &ByteRecord| -> CliResult<()> {
            for output_row in output_rows {
                writer.write_fields(|w| {
                    for cell in sel.select(record) {
                        w.write_field(cell)?;
                    }

                    for value in output_row {
                        w.write_field(serializer.serialize(value))?;
                    }

                    Ok(())
                })?;
            }

            Ok(())
        };

    if let Some(t) = threads {
        inputs
            .enumerate()
            .parallel_map_custom(
                |o| o.threads(t),
                move |(row_index, result)| -> CliResult<(Input, Vec<Vec<DynamicValue>>)> {
                    let input = result?;
                    let output_rows = scraper.scrape_or_report(&input_dir, row_index, &input)?;

                    Ok((input, output_rows))
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let (input, output_rows) = result?;

                write_output(&output_rows, input.as_record())?;

                Ok(())
            })?;
    } else {
        for (row_index, result) in inputs.enumerate() {
            let input = result?;

            let output_rows = scraper.scrape_or_report(&input_dir, row_index, &input)?;

            write_output(&output_rows, input.as_record())?;
        }
    }

    Ok(writer.flush()?)
}
