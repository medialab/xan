use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, Read};
use std::iter;
use std::path::PathBuf;
use std::str::from_utf8;

use bstr::ByteSlice;
use colored::Colorize;
use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use pariter::IteratorExt;
use regex::bytes::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicValue, ScrapingProgram};
use crate::select::{SelectColumns, Selection};
use crate::util;
use crate::CliResult;

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
}

fn looks_like_html(bytes: &[u8]) -> bool {
    HTML_LIKE_REGEX.is_match(bytes)
}

#[derive(Clone)]
struct CustomScraper {
    program: ScrapingProgram,
    foreach: Option<Selector>,
}

impl CustomScraper {
    fn is_plural(&self) -> bool {
        self.foreach.is_some()
    }

    fn scrape(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        target: ScraperTarget,
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

#[derive(Clone)]
enum Scraper {
    Head,
    Urls(Option<usize>),
    Custom(CustomScraper),
}

impl Scraper {
    fn is_plural(&self) -> bool {
        match self {
            Self::Head => false,
            Self::Urls(_) => true,
            Self::Custom(inner) => inner.is_plural(),
        }
    }

    fn names(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self {
            Self::Head => Box::new(["title", "canonical_url"].into_iter()),
            Self::Urls(_) => Box::new(iter::once("url")),
            Self::Custom(scraper) => Box::new(scraper.program.names()),
        }
    }

    fn scrape_head(&self, target: ScraperTarget) -> CliResult<Vec<DynamicValue>> {
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
        target: ScraperTarget,
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

    fn scrape(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        target: ScraperTarget,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        match self {
            Self::Head => Ok(vec![self.scrape_head(target)?]),
            Self::Urls(url_column_index) => {
                let urls = self.scrape_urls(record, target, *url_column_index)?;

                Ok(urls.into_iter().map(|url| vec![url]).collect())
            }
            Self::Custom(scraper) => scraper.scrape(index, record, target),
        }
    }
}

static USAGE: &str = "
Scrape HTML files to output tabular CSV data.

This command can either process a CSV file with a column containing
raw HTML, or a CSV file with a column of relative paths that will
be read by the command for you when using the -I/--input-dir flag:

Scraping a HTML column:

    $ xan scrape title document docs.csv > docs-with-titles.csv

Scraping HTML files on disk, using the -I/--input-dir flag:

    $ xan scrape title path -I ./docs > docs-with-title.csv

Then, this command knows how to scrape typical stuff from HTML such
as titles, urls, images and other metadata using very optimized routines
or can let you define a custom scraper that you can give through
the -e/--evaluate or -f/--evaluate-file.

The command can of course use multiple CPUs to go faster using -p/--parallel
or -t/--threads.

# Builtin scrapers

    - \"head\": scrape typical metadata found in <head> like:
        - \"title\"
        - \"canonical_url\"
    - \"urls\": find all urls linked in the document

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

# Singular or plural?

Scrapers can be \"singular\" or \"plural\".

A singular scraper will produce exactly one output row per input row,
while a plural scraper can produce 0 to n output rows per input row.

Singular builtin scrapers: \"head\".

Plural builtin scrapers: \"urls\".

Custom scrapers are singular, except when using -F/--foreach.

It can be useful, especially when using plural scrapers, to use
the -k/--keep flag to select the input columns to keep in the output.

Usage:
    xan scrape -e <expr> <column> [options] [<input>]
    xan scrape -f <path> <column> [options] [<input>]
    xan scrape head <column> [options] [<input>]
    xan scrape urls <column> [options] [<input>]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>       If given, evaluate the given scraping expression.
    -f, --evaluate-file <path>  If given, evaluate the scraping expression found
                                in file at <path>.
    -I, --input-dir <path>      If given, target column will be understood
                                as relative path to read from this input
                                directory instead.
    -k, --keep <column>         Selection of columns from the input to keep in
                                the output.
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

    let scraper = match &args.flag_evaluate {
        Some(code) => Scraper::Custom(CustomScraper {
            program: ScrapingProgram::parse(code, &headers)?,
            foreach: args
                .flag_foreach
                .as_ref()
                .map(|css| {
                    Selector::parse(css).map_err(|_| format!("invalid CSS selector: {}", css))
                })
                .transpose()?,
        }),
        None => {
            if args.cmd_head {
                Scraper::Head
            } else if args.cmd_urls {
                Scraper::Urls(url_column_index)
            } else {
                unreachable!()
            }
        }
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

    let padding = scraper.names().count();

    if !args.flag_no_headers {
        let mut output_headers = headers.clone();

        if let Some(keep_sel) = &keep {
            output_headers = keep_sel.select(&output_headers).collect();
        }

        for name in scraper.names() {
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

                    let output_rows = scraper.scrape(index, &record, target)?;

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

                    scraper.scrape(index, &record, target)?
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
