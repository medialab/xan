use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;

use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use scraper::{Html, Selector};

use crate::config::{Config, Delimiter};
use crate::moonblade::{DynamicValue, ScrapingProgram};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

lazy_static! {
    static ref TITLE_SELECTOR: Selector = Selector::parse("title").unwrap();
}

// TODO: support for partial read and regex scraping
fn scrape_title(html: &Html) -> Vec<DynamicValue> {
    if let Some(title_node) = html.select(&TITLE_SELECTOR).next() {
        vec![DynamicValue::from(
            title_node.text().collect::<String>().trim(),
        )]
    } else {
        vec![]
    }
}

fn open(input_dir: &str, filename: &str) -> io::Result<String> {
    let mut path = PathBuf::from(input_dir);
    path.push(filename);

    let mut file = File::open(path)?;
    let mut contents = String::new();

    if filename.ends_with(".gz") {
        GzDecoder::new(file).read_to_string(&mut contents)?;
    } else {
        file.read_to_string(&mut contents)?;
    }

    Ok(contents)
}

static USAGE: &str = "
TODO...

Usage:
    xan scrape <column> -e <expr> [options] [<input>]
    xan scrape <column> title [options] [<input>]
    xan scrape --help

scrape options:
    -e, --evaluate <expr>   If given, evaluate the given scraping expression.
    --foreach <css>         If given, will return one row per element matching
                            the CSS selector in target document, instead of returning
                            a single row per document.
    -I, --input-dir <path>  If given, target column will be understood
                            as relative path to read from this input
                            directory instead.
    --keep <column>

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

    let program_opt = args
        .flag_evaluate
        .as_ref()
        .map(|code| ScrapingProgram::parse(code, &headers))
        .transpose()?;

    if program_opt.is_none() && args.flag_foreach.is_some() {
        Err("--foreach only works with -e/--evaluate!")?;
    }

    let foreach_selector = args
        .flag_foreach
        .as_ref()
        .map(|css| Selector::parse(css).map_err(|_| format!("invalid CSS selector: {}", css)))
        .transpose()?;

    let keep = args
        .flag_keep
        .map(|s| s.selection(&headers, !args.flag_no_headers))
        .transpose()?;

    let mut writer = Config::new(&args.flag_output).writer()?;

    if !args.flag_no_headers {
        let mut output_headers = headers.clone();

        if let Some(keep_sel) = &keep {
            output_headers = keep_sel.select(&output_headers).collect();
        }

        if let Some(program) = &program_opt {
            for name in program.names() {
                output_headers.push_field(name.as_bytes());
            }
        } else if args.cmd_title {
            output_headers.push_field(b"title");
        }

        writer.write_byte_record(&output_headers)?;
    }

    let mut record = csv::ByteRecord::new();
    let mut index: usize = 0;

    while reader.read_byte_record(&mut record)? {
        let cell = std::str::from_utf8(&record[column_index]).expect("invalid utf-8");

        let html = if let Some(input_dir) = &args.flag_input_dir {
            Html::parse_document(
                &open(input_dir, cell).map_err(|_| format!("error while opening {}", cell))?,
            )
        } else {
            Html::parse_document(cell)
        };

        // Plural
        if let Some(foreach) = &foreach_selector {
            for result in program_opt
                .as_ref()
                .unwrap()
                .run_plural(index, &record, &html, foreach)
            {
                let values = result?;

                let mut output_record = if let Some(keep_sel) = &keep {
                    keep_sel.select(&record).collect()
                } else {
                    record.clone()
                };

                for value in values {
                    output_record.push_field(&value.serialize_as_bytes_with_options(b"|"));
                }

                writer.write_byte_record(&output_record)?;
            }
        }
        // Singular
        else {
            let values = if let Some(program) = &program_opt {
                program.run_singular(index, &record, &html)?
            } else {
                scrape_title(&html)
            };

            if let Some(keep_sel) = &keep {
                record = keep_sel.select(&record).collect();
            }

            for value in values {
                record.push_field(&value.serialize_as_bytes_with_options(b"|"));
            }

            writer.write_byte_record(&record)?;
        }

        index += 1;
    }

    Ok(writer.flush()?)
}
