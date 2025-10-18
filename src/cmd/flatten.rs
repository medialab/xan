use std::io::{self, Write};
use std::num::NonZeroUsize;

use colored::Colorize;
use regex::{Captures, RegexBuilder};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util;
use crate::CliResult;

static USAGE: &str = "
Prints flattened records such that fields are labeled separated by a new line.
This mode is particularly useful for viewing one record at a time.

There is also a condensed view (-c or --condense) that will shorten the
contents of each field to provide a summary view.

Pipe into \"less -r\" if you need to page the result, and use -C/--force-colors
not to lose the colors:

    $ xan flatten -C file.csv | less -Sr

Usage:
    xan flatten [options] [<input>]
    xan f [options] [<input>]

flatten options:
    -s, --select <arg>     Select the columns to visualize. See 'xan select -h'
                           for the full syntax.
    -l, --limit <n>        Maximum number of rows to read. Defaults to read the whole
                           file.
    -c, --condense         Don't wrap cell values on new lines but truncate them
                           with ellipsis instead.
    -w, --wrap             Wrap cell values all while minding the header's indent.
    -F, --flatter          Even flatter representation alternating column name and content
                           on different lines in the output. Useful to display cells containing
                           large chunks of text.
    --cols <num>           Width of the graph in terminal columns, i.e. characters.
                           Defaults to using all your terminal's width or 80 if
                           terminal's size cannot be found (i.e. when piping to file).
                           Can also be given as a ratio of the terminal's width e.g. \"0.5\".
    -R, --rainbow          Alternating colors for cells, rather than color by value type.
    -C, --force-colors     Force colors even if output is not supposed to be able to
                           handle them.
    -S, --split <cols>     Split columns containing multiple values separated by --sep
                           to be displayed as a list.
    --sep <sep>            Delimiter separating multiple values in cells splitted
                           by --plural. [default: |]
    -H, --highlight <pat>  Highlight in red parts of text cells matching given regex
                           pattern. Will not work with -R/--rainbow.
    -i, --ignore-case      If given, pattern given to -H/--highlight will be case-insensitive.

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
    flag_limit: Option<NonZeroUsize>,
    flag_condense: bool,
    flag_wrap: bool,
    flag_flatter: bool,
    flag_cols: Option<String>,
    flag_rainbow: bool,
    flag_force_colors: bool,
    flag_split: Option<SelectColumns>,
    flag_sep: String,
    flag_highlight: Option<String>,
    flag_ignore_case: bool,
    flag_no_headers: bool,
    flag_delimiter: Option<Delimiter>,
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let args: Args = util::get_args(USAGE, argv)?;

    if args.flag_rainbow && args.flag_highlight.is_some() {
        Err("-R/--rainbow does not work with -H/--highlight!")?;
    }

    let modalities = args.flag_wrap as u8 + args.flag_condense as u8 + args.flag_flatter as u8;

    if modalities > 1 {
        Err("must choose only one of -w/--wrap, -c/--condense or -F/--flatter!")?;
    }

    let output = io::stdout();

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select.clone());
    let mut rdr = rconfig.reader()?;
    let byte_headers = rdr.byte_headers()?;
    let sel = rconfig.selection(byte_headers)?;

    let split_sel_opt = args
        .flag_split
        .map(|cols| {
            cols.selection(
                &sel.select(byte_headers).collect::<csv::ByteRecord>(),
                !args.flag_no_headers,
            )
        })
        .transpose()?;

    let highlight_pattern = args
        .flag_highlight
        .as_ref()
        .map(|pattern| {
            RegexBuilder::new(pattern)
                .case_insensitive(args.flag_ignore_case)
                .build()
        })
        .transpose()?;

    if args.flag_force_colors {
        colored::control::set_override(true);
    }

    let cols = util::acquire_term_cols_ratio(&args.flag_cols)?;

    let potential_headers = rdr.headers()?.clone();
    let potential_headers = sel
        .select(&potential_headers)
        .collect::<csv::StringRecord>();
    let mut headers: Vec<String> = Vec::new();

    for (i, header) in potential_headers.iter().enumerate() {
        let header = match rconfig.no_headers {
            true => i.to_string(),
            false => header.to_string(),
        };
        headers.push(header);
    }

    headers = headers
        .into_iter()
        .map(|name| util::sanitize_text_for_single_line_printing(&name))
        .collect();

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

    let prepare_cell = |i: usize, cell: &str, offset: usize| -> String {
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
                &util::sanitize_text_for_single_line_printing(cell),
                max_value_width.saturating_sub(offset),
                " ",
                true,
            )
        } else if args.flag_wrap {
            util::highlight_problematic_string_features(&util::wrap(
                &util::sanitize_text_for_multi_line_printing(cell),
                max_value_width.saturating_sub(offset),
                max_header_width + 1 + offset,
            ))
        } else {
            util::highlight_problematic_string_features(cell)
        };

        let cell = util::colorize(&cell_colorizer, &cell);

        match (cell_colorizer.highlightable_color(), &highlight_pattern) {
            (Some(fg), Some(pattern)) => pattern
                .replace_all(&cell.to_string(), |caps: &Captures| {
                    let mut r = String::from("\x1b[0;1;31m");
                    r.push_str(&caps[0]);
                    r.push_str("\x1b[0;");
                    r.push_str(&fg);
                    r.push('m');
                    r
                })
                .into_owned(),
            _ => cell.to_string(),
        }
    };

    let display_headers = headers
        .iter()
        .map(|header| {
            util::unicode_aware_highlighted_pad_with_ellipsis(
                false,
                header,
                max_header_width + 1,
                " ",
                true,
            )
        })
        .collect::<Vec<_>>();

    while rdr.read_record(&mut record)? {
        if record_index > 0 {
            writeln!(&output)?;
        }

        writeln!(&output, "{}", format!("Row n°{}", record_index).bold())?;
        writeln!(&output, "{}", "─".repeat(cols).dimmed())?;

        for (i, (header, cell)) in display_headers.iter().zip(sel.select(&record)).enumerate() {
            // Splitted cell
            if matches!(&split_sel_opt, Some(split_sel) if !cell.is_empty() && split_sel.contains(i))
            {
                let mut first: bool = true;

                write!(&output, "{}", header)?;

                for sub_cell in cell.split(&args.flag_sep) {
                    let sub_cell = prepare_cell(i, sub_cell, 2);

                    if first {
                        first = false;
                        writeln!(&output, "- {}", sub_cell)?;
                    } else {
                        writeln!(
                            &output,
                            "{}- {}",
                            " ".repeat(max_header_width + 1),
                            sub_cell
                        )?;
                    }
                }

                writeln!(&output)?;

                continue;
            }

            // Regular cell
            let cell = prepare_cell(i, cell, 0);

            if args.flag_flatter {
                writeln!(&output, "{}", header)?;
                writeln!(&output, "{}\n", cell)?;
            } else {
                writeln!(&output, "{}{}", header, cell)?;
            }
        }

        record_index += 1;

        if let Some(limit) = args.flag_limit {
            if record_index >= limit.get() {
                break;
            }
        }
    }

    Ok(())
}
