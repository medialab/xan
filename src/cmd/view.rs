use std::env;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::Path;
use std::str::FromStr;

use colored::{self, Colorize};
use numfmt::{Formatter, Precision};
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::select::SelectColumns;
use crate::util::{self, ImmutableRecordHelpers};
use crate::CliResult;

const HEADERS_ROWS: usize = 8;

type BoxCharsArray = [char; 11];

#[repr(u8)]
enum BoxChar {
    CornerUpLeft,
    CornerUpRight,
    CornerBottomLeft,
    CornerBottomRight,
    CrossLeft,
    CrossRight,
    CrossBottom,
    CrossUp,
    CrossFull,
    Horizontal,
    Vertical,
}

const BOX_CHARS: BoxCharsArray = ['┌', '┐', '└', '┘', '┤', '├', '┬', '┴', '┼', '─', '│'];
const ROUNDED_BOX_CHARS: BoxCharsArray = ['╭', '╮', '╰', '╯', '┤', '├', '┬', '┴', '┼', '─', '│'];
const INVISIBLE_BOX_CHARS: BoxCharsArray = [' '; 11];

struct ViewTheme {
    padding: &'static str,
    index_column_header: &'static str,
    box_chars: BoxCharsArray,
    hr_under_headers: bool,
    external_borders: bool,
    striped: bool,
}

impl Default for ViewTheme {
    fn default() -> Self {
        Self {
            padding: " ",
            index_column_header: "-",
            box_chars: BOX_CHARS,
            hr_under_headers: true,
            external_borders: true,
            striped: false,
        }
    }
}

impl ViewTheme {
    // Themes beyond default
    fn borderless() -> Self {
        Self {
            index_column_header: " ",
            box_chars: INVISIBLE_BOX_CHARS,
            hr_under_headers: false,
            external_borders: false,
            ..Self::default()
        }
    }

    fn compact() -> Self {
        Self {
            padding: "",
            index_column_header: " ",
            box_chars: INVISIBLE_BOX_CHARS,
            hr_under_headers: false,
            external_borders: false,
            ..Self::default()
        }
    }

    fn rounded() -> Self {
        Self {
            box_chars: ROUNDED_BOX_CHARS,
            ..Self::default()
        }
    }

    fn slim() -> Self {
        Self {
            index_column_header: " ",
            external_borders: false,
            ..Self::default()
        }
    }

    fn striped() -> Self {
        Self {
            padding: "",
            index_column_header: " ",
            box_chars: INVISIBLE_BOX_CHARS,
            hr_under_headers: false,
            external_borders: false,
            striped: true,
        }
    }

    // Methods
    #[inline]
    fn horizontal_box(&self) -> String {
        self.box_chars[BoxChar::Horizontal as usize].to_string()
    }

    #[inline]
    fn corner_up_left(&self) -> char {
        self.box_chars[BoxChar::CornerUpLeft as usize]
    }

    #[inline]
    fn corner_up_right(&self) -> char {
        self.box_chars[BoxChar::CornerUpRight as usize]
    }

    #[inline]
    fn corner_bottom_left(&self) -> char {
        self.box_chars[BoxChar::CornerBottomLeft as usize]
    }

    #[inline]
    fn corner_bottom_right(&self) -> char {
        self.box_chars[BoxChar::CornerBottomRight as usize]
    }

    #[inline]
    fn cross_left(&self) -> char {
        self.box_chars[BoxChar::CrossLeft as usize]
    }

    #[inline]
    fn cross_right(&self) -> char {
        self.box_chars[BoxChar::CrossRight as usize]
    }

    #[inline]
    fn cross_bottom(&self) -> char {
        self.box_chars[BoxChar::CrossBottom as usize]
    }

    #[inline]
    fn cross_up(&self) -> char {
        self.box_chars[BoxChar::CrossUp as usize]
    }

    #[inline]
    fn cross_full(&self) -> char {
        self.box_chars[BoxChar::CrossFull as usize]
    }

    #[inline]
    fn vertical(&self) -> char {
        self.box_chars[BoxChar::Vertical as usize]
    }
}

impl FromStr for ViewTheme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "table" => Self::default(),
            "borderless" => Self::borderless(),
            "compact" => Self::compact(),
            "rounded" => Self::rounded(),
            "slim" => Self::slim(),
            "striped" => Self::striped(),
            _ => return Err(format!("unknown \"{}\" theme!", s)),
        })
    }
}

static USAGE: &str = "
Preview CSV data in the terminal in a human-friendly way with aligned columns,
shiny colors & all.

The command will by default try to display as many columns as possible but
will truncate cells/columns to avoid overflowing available terminal screen.

If you want to display all the columns using a pager, prefer using
the -p/--pager flag that internally rely on the ubiquitous \"less\"
command.

If you still want to use a pager manually, don't forget to use
the -e/--expand and -C/--force-colors flags before piping like so:

    $ xan view -eC file.csv | less -SR

Finally, it is possible to customize the default behavior of this command through
the \"XAN_VIEW_ARGS\" environment variable. This variable takes a series of
supported flags: -t/--theme, -p/--pager, -l/--limit, -R/--rainbow, -E/--sanitize-emojis,
and -S/--significance, -I/--hide-index.

So if you want, for instance, to use the borderles theme, hide the index column and
restrict the number of floating points decimals to be shown by default:

    $ XAN_VIEW_ARGS=\"-t borderless -S 5 -I\"

Usage:
    xan view [options] [<input>]
    xan v [options] [<input>]
    xan view --help

view options:
    -s, --select <arg>      Select the columns to visualize. See 'xan select -h'
                            for the full syntax.
    -t, --theme <name>      Theme for the table display, one of: \"table\", \"borderless\",
                            \"compact\", \"rounded\", \"slim\" or \"striped\".
                            [default: table]
    -p, --pager             Automatically use the \"less\" command to page the results.
                            This flag does not work on windows!
    -A, --all               Remove the row limit and display everything.
    -l, --limit <number>    Maximum of rows to read into memory. Use -A, --all or
                            set to 0 to disable the limit.
                            [default: 100]
    -R, --rainbow           Alternating colors for columns, rather than color by value type.
    --cols <num>            Width of the graph in terminal columns, i.e. characters.
                            Defaults to using all your terminal's width or 80 if
                            terminal's size cannot be found (i.e. when piping to file).
                            Can also be given as a ratio of the terminal's width e.g. \"0.5\".
    -C, --force-colors      Force colors even if output is not supposed to be able to
                            handle them.
    -e, --expand            Expand the table so that in can be easily piped to
                            a pager such as \"less\", with larger width constraints.
    -E, --sanitize-emojis   Replace emojis by their shortcode to avoid formatting issues.
    -S, --significance <n>  Maximum floating point significance used to format numbers.
    -I, --hide-index        Hide the row index on the left.
    -H, --hide-headers      Hide the headers. Implied when -n, --no-headers is given.
    -M, --hide-info         Hide information about number of displayed columns, rows etc.
    -g, --groupby <cols>    Isolate and emphasize groups of rows, represented by consecutive
                            rows with identical values in selected columns.

Common options:
    -h, --help             Display this message
    -n, --no-headers       When set, the first row will not considered as being
                           the file header.
    -d, --delimiter <arg>  The field delimiter for reading CSV data.
                           Must be a single character.
";

#[derive(Deserialize, Debug)]
struct Args {
    arg_input: Option<String>,
    flag_select: SelectColumns,
    flag_pager: bool,
    flag_theme: String,
    flag_cols: Option<String>,
    flag_delimiter: Option<Delimiter>,
    flag_no_headers: bool,
    flag_force_colors: bool,
    flag_all: bool,
    flag_limit: usize,
    flag_rainbow: bool,
    flag_expand: bool,
    flag_sanitize_emojis: bool,
    flag_hide_index: bool,
    flag_hide_headers: bool,
    flag_hide_info: bool,
    flag_groupby: Option<SelectColumns>,
    flag_significance: Option<NonZeroUsize>,
}

impl Args {
    fn resolve(&mut self) {
        if self.flag_all {
            self.flag_limit = 0;
        }

        if self.flag_no_headers {
            self.flag_hide_headers = true;
        }
    }

    fn infer_expand(&self) -> bool {
        self.flag_pager || self.flag_expand
    }

    fn infer_force_colors(&self) -> bool {
        self.flag_pager || self.flag_force_colors
    }

    fn merge(from_env: Self, mut from_argv: Self) -> Self {
        if from_argv.flag_theme == "table" && from_env.flag_theme != "table" {
            from_argv.flag_theme = from_env.flag_theme;
        }

        if !from_argv.flag_hide_index && from_env.flag_hide_index {
            from_argv.flag_hide_index = true;
        }

        if !from_argv.flag_pager && from_env.flag_pager {
            from_argv.flag_pager = true;
        }

        if !from_argv.flag_rainbow && from_env.flag_rainbow {
            from_argv.flag_rainbow = true;
        }

        if !from_argv.flag_sanitize_emojis && from_env.flag_sanitize_emojis {
            from_argv.flag_sanitize_emojis = true;
        }

        if from_argv.flag_limit == 100 && from_env.flag_limit != 100 {
            from_argv.flag_limit = from_env.flag_limit;
        }

        if from_argv.flag_significance.is_none() && from_env.flag_significance.is_some() {
            from_argv.flag_significance = from_env.flag_significance;
        }

        from_argv
    }
}

pub fn run(argv: &[&str]) -> CliResult<()> {
    let config_path_env =
        env::var_os("XDG_CONFIG_HOME").map(|xdg| Path::new(&xdg).join("xan/config.toml"));

    let config_args: Vec<String> = if let Some(config_path) = config_path_env {
        if config_path.exists() {
            let file_config: util::FileConfig = util::load_config(config_path)?;
            file_config.view.flags.clone()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    let mut merged_args: Vec<&str> = argv.to_vec();
    merged_args.extend(config_args.iter().map(|s| s.as_str()));
    let mut args: Args = util::get_args(USAGE, &merged_args)?;
    args.resolve();

    let mut env_var_argv = vec!["xan", "view"];
    let env_var_split =
        shlex::split(&env::var("XAN_VIEW_ARGS").unwrap_or("".to_string())).unwrap_or_default();

    for env_arg in env_var_split.iter() {
        env_var_argv.push(env_arg);
    }

    let mut env_args: Args = util::get_args(USAGE, &env_var_argv)?;
    env_args.resolve();

    let args = Args::merge(env_args, args);

    if args.infer_force_colors() {
        colored::control::set_override(true);
    }

    let emoji_sanitizer = util::EmojiSanitizer::new();

    let output = io::stdout();

    let cols = util::acquire_term_cols_ratio(&args.flag_cols)?;
    let rows = termsize::get().map(|size| size.rows as usize);

    // Theme
    let theme = args.flag_theme.parse::<ViewTheme>()?;

    let padding = theme.padding;
    let horizontal_box = theme.horizontal_box();

    let rconfig = Config::new(&args.arg_input)
        .delimiter(args.flag_delimiter)
        .no_headers(args.flag_no_headers)
        .select(args.flag_select.clone());

    let mut rdr = rconfig.reader()?;
    let byte_headers = rdr.byte_headers()?;
    let sel = rconfig.selection(byte_headers)?;

    let mut groupby_sel_opt = args
        .flag_groupby
        .clone()
        .map(|cols| cols.selection(byte_headers, !args.flag_no_headers))
        .transpose()?;

    if let (Some(groupby_sel), false) = (&mut groupby_sel_opt, args.flag_hide_index) {
        groupby_sel.offset_by(1);
    }

    let headers = rdr.headers()?.clone();
    let mut headers = sel
        .select_string_record(&headers)
        .collect::<csv::StringRecord>();

    if !args.flag_hide_index {
        headers = headers.prepend(theme.index_column_header);
    }

    if rconfig.no_headers {
        headers = headers
            .into_iter()
            .enumerate()
            .map(|(i, h)| {
                if args.flag_hide_index {
                    i.to_string()
                } else if i == 0 {
                    h.to_string()
                } else {
                    (i - 1).to_string()
                }
            })
            .collect();
    }

    let mut all_records_buffered = false;

    let mut number_formatter = args.flag_significance.map(|s| {
        Formatter::new()
            .precision(Precision::Significance(s.get() as u8))
            .separator(',')
            .unwrap()
    });

    let records = {
        let limit = args.flag_limit;

        let mut r_iter = rdr.into_records().enumerate();

        let mut records: Vec<csv::StringRecord> = Vec::new();

        loop {
            match r_iter.next() {
                None => break,
                Some((i, record)) => {
                    let mut record = sel
                        .select_string_record(&record?)
                        .map(|cell| {
                            let mut cell = cell.to_string();

                            cell = util::sanitize_text_for_single_line_printing(&cell);

                            if args.flag_sanitize_emojis {
                                cell = emoji_sanitizer.sanitize(&cell);
                            }

                            if let Some(fmt) = number_formatter.as_mut() {
                                if let Ok(f) = cell.parse::<f64>() {
                                    cell = util::format_number_with_formatter(fmt, f);
                                }
                            }

                            cell
                        })
                        .collect::<csv::StringRecord>();

                    if !args.flag_hide_index {
                        record = record.prepend(&i.to_string());
                    }

                    records.push(record);

                    if limit > 0 && records.len() == limit {
                        break;
                    }
                }
            };
        }

        if r_iter.next().is_none() {
            all_records_buffered = true;
        }

        records
    };

    let need_to_repeat_headers = match rows {
        None => true,
        Some(r) => records.len() + HEADERS_ROWS > r,
    };

    let max_column_widths: Vec<usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            usize::max(
                if args.flag_hide_headers { 0 } else { h.width() },
                records
                    .iter()
                    .map(|c| match c[i].width() {
                        0 => 7, // NOTE: taking <empty> into account
                        v => v,
                    })
                    .max()
                    .unwrap_or(0),
            )
        })
        .collect();

    // Width inferrence
    let displayed_columns = infer_best_column_display(
        cols,
        &max_column_widths,
        args.infer_expand(),
        if args.flag_hide_index { 0 } else { 1 },
        padding,
    );

    let all_columns_shown = displayed_columns.len() == headers.len();

    // NOTE: we setup the pager when everything has been read and process and no error
    // occurred along the way, so that we don't get to read a paged error
    if args.flag_pager {
        #[cfg(not(windows))]
        {
            pager::Pager::with_pager("less -SR").setup();
        }

        #[cfg(windows)]
        {
            Err("The -p/--pager flag does not work on windows, sorry :'(".to_string())?;
        }
    }

    let write_info = || -> Result<(), io::Error> {
        if args.flag_hide_info {
            return Ok(());
        }

        let len_offset = if args.flag_hide_index { 0 } else { 1 };

        let pretty_records_len = util::format_number(records.len());
        let pretty_headers_len = util::format_number(headers.len() - len_offset);
        let pretty_displayed_headers_len =
            util::format_number(displayed_columns.len() - len_offset);

        writeln!(
            &output,
            "Displaying {} col{} from {} of {}",
            if all_columns_shown {
                format!("{}", pretty_headers_len.cyan())
            } else {
                format!(
                    "{}/{}",
                    pretty_displayed_headers_len.cyan(),
                    pretty_headers_len.cyan(),
                )
            },
            if headers.len() > 2 { "s" } else { "" },
            if all_records_buffered {
                format!("{} rows", pretty_records_len.cyan())
            } else {
                format!("{} first rows", pretty_records_len.cyan())
            },
            match &args.arg_input {
                Some(filename) => filename,
                None => "<stdin>",
            }
            .dimmed()
        )?;

        Ok(())
    };

    enum HRPosition {
        Top,
        Middle,
        Bottom,
    }

    let write_horizontal_ruler = |pos: HRPosition| -> Result<(), io::Error> {
        let mut s = String::new();

        if theme.external_borders {
            s.push(match pos {
                HRPosition::Bottom => theme.corner_up_left(),
                HRPosition::Top => theme.corner_bottom_left(),
                HRPosition::Middle => theme.cross_right(),
            });
        }

        displayed_columns.iter().enumerate().for_each(|(i, col)| {
            s.push_str(&horizontal_box.repeat(
                col.allowed_width + 2 * padding.len()
                    - (if i == 0 && !theme.external_borders {
                        1
                    } else {
                        0
                    }),
            ));

            if !all_columns_shown && Some(i) == displayed_columns.split_point() {
                s.push(match pos {
                    HRPosition::Bottom => theme.cross_bottom(),
                    HRPosition::Top => theme.cross_up(),
                    HRPosition::Middle => theme.cross_full(),
                });

                s.push_str(&horizontal_box.repeat(1 + 2 * padding.len()));
            }

            if i == displayed_columns.len() - 1 {
                return;
            }

            s.push(match pos {
                HRPosition::Bottom => theme.cross_bottom(),
                HRPosition::Top => theme.cross_up(),
                HRPosition::Middle => theme.cross_full(),
            });
        });

        if theme.external_borders {
            s.push(match pos {
                HRPosition::Bottom => theme.corner_up_right(),
                HRPosition::Top => theme.corner_bottom_right(),
                HRPosition::Middle => theme.cross_left(),
            });
        }

        writeln!(&output, "{}", s.dimmed())?;

        Ok(())
    };

    let write_row = |row: Vec<colored::ColoredString>, mut dimmed: bool| -> Result<(), io::Error> {
        if !theme.striped {
            dimmed = false;
        }

        if theme.external_borders {
            write!(
                &output,
                "{}",
                format!("{}{}", theme.vertical(), padding).dimmed()
            )?;
        }

        for (i, cell) in row.iter().enumerate() {
            if i != 0 {
                write!(
                    &output,
                    "{}",
                    format!("{}{}{}", padding, theme.vertical(), padding).dimmed()
                )?;
            }

            if dimmed {
                write!(&output, "{}", cell.clone().reversed())?;
            } else {
                write!(&output, "{}", cell)?;
            }

            if !all_columns_shown && Some(i) == displayed_columns.split_point() {
                write!(
                    &output,
                    "{}",
                    format!("{}{}{}…", padding, theme.vertical(), padding).dimmed(),
                )?;
            }
        }

        if theme.external_borders {
            write!(
                &output,
                "{}",
                format!("{}{}", padding, theme.vertical()).dimmed()
            )?;
        }

        writeln!(&output)?;

        Ok(())
    };

    let write_headers = |above: bool| -> Result<(), io::Error> {
        if above || theme.hr_under_headers {
            write_horizontal_ruler(if above {
                HRPosition::Bottom
            } else {
                HRPosition::Middle
            })?;
        }

        let headers_row: Vec<colored::ColoredString> = displayed_columns
            .iter()
            .map(|col| (col, &headers[col.index]))
            .enumerate()
            .map(|(i, (col, h))| {
                let cell = util::unicode_aware_rpad_with_ellipsis(h, col.allowed_width, " ");

                if !args.flag_hide_index && i == 0 {
                    cell.dimmed()
                } else {
                    cell.bold()
                }
            })
            .collect();

        write_row(headers_row, false)?;

        if !above || theme.hr_under_headers {
            write_horizontal_ruler(if above {
                HRPosition::Middle
            } else {
                HRPosition::Top
            })?;
        }

        Ok(())
    };

    writeln!(&output)?;
    write_info()?;

    // NOTE: we stop if there is nothing to show
    let nothing_to_show =
        records.is_empty() && (headers.is_empty() || (!args.flag_hide_index && headers.len() == 1));

    if nothing_to_show {
        return Ok(());
    }

    if args.flag_hide_headers {
        write_horizontal_ruler(HRPosition::Bottom)?;
    } else {
        write_headers(true)?;
    }

    let mut last_group: Option<Vec<String>> = None;
    let mut record_i: usize = 0;

    for record in records.iter() {
        let (need_to_draw_hr, need_to_erase_sel) = if let Some(groupby_sel) = &groupby_sel_opt {
            let current_key = groupby_sel
                .select_string_record(record)
                .map(|cell| cell.to_string())
                .collect::<Vec<_>>();

            match &last_group {
                None => {
                    last_group = Some(current_key);
                    (false, false)
                }
                Some(last_key) if last_key != &current_key => {
                    last_group = Some(current_key);
                    (true, false)
                }
                _ => (false, true),
            }
        } else {
            (false, false)
        };

        if need_to_draw_hr {
            write_horizontal_ruler(HRPosition::Middle)?;
        }

        let row: Vec<colored::ColoredString> = displayed_columns
            .iter()
            .map(|col| (col, &record[col.index]))
            .enumerate()
            .map(|(i, (col, cell))| {
                if let Some(groupby_sel) = &groupby_sel_opt {
                    if need_to_erase_sel && groupby_sel.contains(i) {
                        return " ".repeat(col.allowed_width).normal();
                    }
                }

                let cell = match cell.trim() {
                    "" => "<empty>",
                    _ => cell,
                };

                let colorizer = if args.flag_rainbow {
                    util::colorizer_by_rainbow(i, cell)
                } else {
                    util::colorizer_by_type(cell)
                };

                if !args.flag_hide_index && i == 0 {
                    util::unicode_aware_rpad_with_ellipsis(cell, col.allowed_width, " ").dimmed()
                } else {
                    util::colorize(
                        &colorizer,
                        &util::unicode_aware_highlighted_pad_with_ellipsis(
                            false,
                            cell,
                            col.allowed_width,
                            " ",
                            true,
                        ),
                    )
                }
            })
            .collect();

        write_row(row, record_i % 2 == 0)?;
        record_i += 1;
    }

    if !all_records_buffered {
        let row: Vec<colored::ColoredString> = displayed_columns
            .iter()
            .map(|col| util::unicode_aware_rpad_with_ellipsis("…", col.allowed_width, " ").dimmed())
            .collect();

        write_row(row, record_i % 2 == 0)?;
    }

    if need_to_repeat_headers {
        if args.flag_hide_headers {
            write_horizontal_ruler(HRPosition::Top)?;
        } else {
            write_headers(false)?;
        }
        write_info()?;
        writeln!(&output)?;
    } else {
        write_horizontal_ruler(HRPosition::Top)?;
        writeln!(&output)?;
    }

    Ok(())
}

fn adjust_column_widths(widths: &[usize], max_width: usize) -> Vec<usize> {
    widths.iter().map(|m| usize::min(*m, max_width)).collect()
}

#[derive(Debug)]
struct DisplayedColumn {
    index: usize,
    allowed_width: usize,
    max_width: usize,
}

#[derive(Debug)]
struct DisplayedColumns {
    max_allowed_cols: usize,
    left: Vec<DisplayedColumn>,
    // NOTE: columns are inserted into right in reversed order
    right: Vec<DisplayedColumn>,
}

impl DisplayedColumns {
    fn new(max_allowed_cols: usize) -> Self {
        DisplayedColumns {
            max_allowed_cols,
            left: Vec::new(),
            right: Vec::new(),
        }
    }

    fn split_point(&self) -> Option<usize> {
        self.left.last().map(|col| col.index)
    }

    fn from_widths(cols: usize, widths: Vec<usize>) -> Self {
        let left = widths
            .iter()
            .copied()
            .enumerate()
            .map(|(i, w)| DisplayedColumn {
                index: i,
                allowed_width: w,
                max_width: w,
            })
            .collect::<Vec<_>>();

        DisplayedColumns {
            max_allowed_cols: cols,
            left,
            right: Vec::new(),
        }
    }

    fn len(&self) -> usize {
        self.left.len() + self.right.len()
    }

    fn fitting_count(&self) -> usize {
        self.iter()
            .filter(|col| col.allowed_width == col.max_width)
            .count()
    }

    fn push(&mut self, left: bool, index: usize, allowed_width: usize, max_width: usize) {
        let col = DisplayedColumn {
            index,
            allowed_width,
            max_width,
        };

        if left {
            self.left.push(col);
        } else {
            self.right.push(col);
        }
    }

    fn iter(&self) -> DisplayedColumnsIter {
        DisplayedColumnsIter {
            iter_left: self.left.iter(),
            iter_right: self.right.iter(),
        }
    }
}

struct DisplayedColumnsIter<'a> {
    iter_left: std::slice::Iter<'a, DisplayedColumn>,
    iter_right: std::slice::Iter<'a, DisplayedColumn>,
}

impl<'a> Iterator for DisplayedColumnsIter<'a> {
    type Item = &'a DisplayedColumn;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter_left
            .next()
            .or_else(|| self.iter_right.next_back())
    }
}

// NOTE: greedy way to find best ratio for columns
// We basically test a range of dividers based on the number of columns in the
// CSV file and we try to find the organization optimizing the number of columns
// fitting perfectly, then the number of columns displayed.
fn infer_best_column_display(
    cols: usize,
    max_column_widths: &[usize],
    expand: bool,
    left_advantage: usize,
    padding: &str,
) -> DisplayedColumns {
    if expand {
        // NOTE: we keep max column size to 3/4 of current screen
        return DisplayedColumns::from_widths(
            cols,
            adjust_column_widths(max_column_widths, ((cols as f64) * 0.75) as usize),
        );
    }

    let per_cell_padding_cols = padding.len() * 2 + 1;
    let ellipsis_padding_cols = padding.len() * 4 + 4;

    let mut attempts: Vec<DisplayedColumns> = Vec::new();

    // NOTE: we could also proceed by col increments rather than dividers I suppose
    let extra_dividers = [1.05, 1.1, 2.5];

    let mut dividers = extra_dividers
        .iter()
        .copied()
        .chain((1..=max_column_widths.len()).map(|d| d as f64))
        .collect::<Vec<_>>();

    dividers.sort_by(|a, b| a.total_cmp(b));

    // TODO: this code can be greatly optimized and early break
    // NOTE: here we iteratively test for a range of max width being a division
    // of the term width. But we could also test for an increasing number of
    // columns, all while respecting the width proportion of each column compared
    // to the other selected ones.
    for divider in dividers {
        let max_allowed_width = (cols as f64 / divider) as usize;

        // If we don't have reasonable space we break
        if max_allowed_width <= 3 {
            break;
        }

        let mut attempt = DisplayedColumns::new(max_allowed_width);

        let widths = adjust_column_widths(max_column_widths, max_allowed_width);

        let mut col_budget = cols - ellipsis_padding_cols;
        let mut widths_iter = widths.iter().enumerate();
        let mut toggle = true;
        let mut left_leaning = left_advantage;

        loop {
            let value = if toggle {
                widths_iter
                    .next()
                    .map(|step| (step, true))
                    .or_else(|| widths_iter.next_back().map(|step| (step, false)))
            } else {
                widths_iter
                    .next_back()
                    .map(|step| (step, false))
                    .or_else(|| widths_iter.next().map(|step| (step, true)))
            };

            if let Some(((i, column_width), left)) = value {
                // NOTE: we favor left-leaning columns because of
                // the index column or just for aesthetical reasons
                if left_leaning > 0 {
                    left_leaning -= 1;
                } else {
                    toggle = !toggle;
                }

                if col_budget == 0 {
                    break;
                }

                if *column_width + per_cell_padding_cols > col_budget {
                    if col_budget > 7 {
                        attempt.push(left, i, col_budget, max_column_widths[i]);
                    }
                    break;
                }

                col_budget -= column_width + per_cell_padding_cols;
                attempt.push(left, i, *column_width, max_column_widths[i]);
            } else {
                break;
            }
        }

        attempts.push(attempt);
    }

    // NOTE: we sort by number of columns fitting perfectly, then number of
    // columns we can display, then the maximum cols one cell can have
    attempts
        .into_iter()
        .max_by_key(|a| (a.fitting_count(), a.len(), a.max_allowed_cols))
        .unwrap()
}
