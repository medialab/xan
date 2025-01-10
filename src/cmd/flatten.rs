use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;
use colored;
use colored::Colorize;
use unicode_width::UnicodeWidthStr;

static USAGE: &str = "
Prints flattened records such that fields are labeled separated by a new line.
This mode is particularly useful for viewing one record at a time.

There is also a condensed view (-c or --condense) that will shorten the
contents of each field to provide a summary view.

Pipe into \"less -r\" if you need to page the result, and use \"-C, --force-colors\"
not to lose the colors:

    $ xan flatten -C file.csv | less -r

Usage:
    xan flatten [options] [<input>]
    xan f [options] [<input>]

flatten options:
    -s, --select <arg>     Select the columns to visualize. See 'xan select -h'
                           for the full syntax.
    -c, --condense         Don't wrap cell values on new lines but truncate them
                           with ellipsis instead.
    -w, --wrap             Wrap cell values all while minding the header's indent.
    --cols <num>           Width of the graph in terminal columns, i.e. characters.
                           Defaults to using all your terminal's width or 80 if
                           terminal's size cannot be found (i.e. when piping to file).
    -R, --rainbow          Alternating colors for cells, rather than color by value type.
    -C, --force-colors     Force colors even if output is not supposed to be able to
                           handle them.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not be interpreted
                           as headers. When set, the name of each field
                           will be its index.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_condense: bool,
    flag_wrap: bool,
    flag_cols: Option<usize>,
    flag_rainbow: bool,
    flag_force_colors: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;
    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select.clone());
    let mut rdr = rconfig.reader()?;
    let byte_headers = rdr.byte_headers()?;
    let sel = rconfig.selection(byte_headers)?;

    if args.flag_force_colors {
        colored::control::set_override(true);
    }

    let cols = util::acquire_term_cols(&args.flag_cols);

    let potential_headers = rdr.headers()?.clone();
    let potential_headers = sel
        .select_string_record(&potential_headers)
        .collect::<csv::StringRecord>();
    let mut headers: Vec<String> = Vec::new();

    for (i, header) in potential_headers.iter().enumerate() {
        let header = match rconfig.no_headers {
            true => i.to_string(),
            false => header.to_string(),
        };
        headers.push(header);
    }

    let max_header_width = headers
        .iter()
        .map(|h| h.width())
        .max()
        .ok_or("file is empty")?;

    if cols < max_header_width + 2 {
        Err("not enough cols provided to safely print data!")?;
    }

    let mut record = csv::StringRecord::new();
    let mut record_index: usize = 0;

    let max_value_width = cols - max_header_width - 1;

    while rdr.read_record(&mut record)? {
        let record = sel
            .select_string_record(&record)
            .collect::<csv::StringRecord>();
        if record_index > 0 {
            println!();
        }
        println!("{}", format!("Row n°{}", record_index).bold());
        println!("{}", "─".repeat(cols).dimmed());

        for (i, (header, cell)) in headers.iter().zip(record.iter()).enumerate() {
            let cell = match cell.trim() {
                "" => "<empty>",
                _ => cell,
            };

            let cell_colorizer = if args.flag_rainbow {
                util::colorizer_by_rainbow(i, cell)
            } else {
                util::colorizer_by_type(cell)
            };

            let cell = if args.flag_condense {
                util::unicode_aware_highlighted_pad_with_ellipsis(
                    false,
                    cell,
                    max_value_width,
                    " ",
                    true,
                )
            } else if args.flag_wrap {
                util::unicode_aware_wrap(cell, max_value_width, max_header_width + 1)
            } else {
                util::highlight_trimmable_whitespace(cell)
            };

            let cell = util::colorize(&cell_colorizer, &cell);

            println!(
                "{}{}",
                util::unicode_aware_rpad(header, max_header_width + 1, " "),
                cell
            );
        }

        record_index += 1;
    }

    Ok(())
}
