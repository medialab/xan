use std::fs::File;
use std::io::{self, Read};
use std::iter;
use std::path::PathBuf;

use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use pariter::IteratorExt;
use scraper::{Html, Selector};

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

fn read_to_string(input_dir: &str, filename: &str) -> io::Result<String> {
    let mut string = String::new();

    open(input_dir, filename)?.read_to_string(&mut string)?;

    Ok(string)
}

fn read_to_html(input_dir: &str, filename: &str) -> io::Result<Html> {
    read_to_string(input_dir, filename).map(|string| Html::parse_document(&string))
}

enum ScraperTarget<'a> {
    HtmlCell(&'a str),
    HtmlFile(&'a str, &'a str),
}

impl ScraperTarget<'_> {
    fn read_to_html(&self) -> CliResult<Html> {
        match self {
            Self::HtmlCell(cell) => Ok(Html::parse_document(cell)),
            Self::HtmlFile(input_dir, filename) => Ok(read_to_html(input_dir, filename)?),
        }
    }
}

// Scraper abstractions
lazy_static! {
    static ref TITLE_SELECTOR: Selector = Selector::parse("title").unwrap();
}

struct CustomScraper {
    program: ScrapingProgram,
    foreach: Option<Selector>,
}

impl CustomScraper {
    fn scrape(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        target: ScraperTarget,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        let html = target.read_to_html()?;

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

// TODO: support for partial read and regex scraping
enum Scraper {
    Title,
    Custom(CustomScraper),
}

impl Scraper {
    fn scrape_title(&self, target: ScraperTarget) -> CliResult<Option<DynamicValue>> {
        let html = target.read_to_html()?;

        Ok(html
            .select(&TITLE_SELECTOR)
            .next()
            .map(|title_node| DynamicValue::from(title_node.text().collect::<String>().trim())))
    }

    fn names(&self) -> Box<dyn Iterator<Item = &str> + '_> {
        match self {
            Self::Title => Box::new(iter::once("title")),
            Self::Custom(scraper) => Box::new(scraper.program.names()),
        }
    }

    fn scrape(
        &self,
        index: usize,
        record: &csv::ByteRecord,
        target: ScraperTarget,
    ) -> CliResult<Vec<Vec<DynamicValue>>> {
        match self {
            Self::Title => {
                let title_opt = self.scrape_title(target)?;

                Ok(vec![vec![title_opt.unwrap_or(DynamicValue::None)]])
            }
            Self::Custom(scraper) => scraper.scrape(index, record, target),
        }
    }
}

static USAGE: &str = "
TODO...

Usage:
    xan scrape <column> -e <expr> [options] [<input>]
    xan scrape <column> title [options] [<input>]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>    If given, evaluate the given scraping expression.
    --foreach <css>          If given, will return one row per element matching
                             the CSS selector in target document, instead of returning
                             a single row per document.
    -I, --input-dir <path>   If given, target column will be understood
                             as relative path to read from this input
                             directory instead.
    --keep <column>          Selection of columns from the input to keep in
                             the output.
    -p, --parallel           Whether to use parallelization to speed up computations.
                             Will automatically select a suitable number of threads to use
                             based on your number of cores. Use -t, --threads if you want to
                             indicate the number of threads yourself.
    -t, --threads <threads>  Parellize computations using this many threads. Use -p, --parallel
                             if you want the number of threads to be automatically chosen instead.

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
    cmd_title: bool,
    flag_delimiter: Option<Delimiter>,
    flag_output: Option<String>,
    flag_no_headers: bool,
    flag_evaluate: Option<String>,
    flag_foreach: Option<String>,
    flag_input_dir: Option<String>,
    flag_keep: Option<SelectColumns>,
    flag_parallel: bool,
    flag_threads: Option<usize>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
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
            if args.cmd_title {
                Scraper::Title
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
                |(index, result)| -> CliResult<csv::ByteRecord> {
                    let record = result?;

                    Ok(record)
                },
            )
            .try_for_each(|result| -> CliResult<()> {
                let record = result?;
                Ok(())
            })?;
    } else {
        let mut record = csv::ByteRecord::new();
        let mut index: usize = 0;

        while reader.read_byte_record(&mut record)? {
            let cell = std::str::from_utf8(&record[column_index]).expect("invalid utf-8");

            let target = if let Some(input_dir) = &args.flag_input_dir {
                ScraperTarget::HtmlFile(input_dir, cell)
            } else {
                ScraperTarget::HtmlCell(cell)
            };

            for output_row in scraper.scrape(index, &record, target)? {
                let mut output_record = if let Some(keep_sel) = &keep {
                    keep_sel.select(&record).collect()
                } else {
                    record.clone()
                };

                for value in output_row {
                    output_record.push_field(&value.serialize_as_bytes_with_options(b"|"));
                }

                writer.write_byte_record(&output_record)?;
            }

            index += 1;
        }
    }

    Ok(writer.flush()?)
}
