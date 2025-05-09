use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::path::Path;
use std::process::Command;
use std::str;
use std::thread;
use std::time;

use colored::{Color, ColoredString, Colorize, Styles};
use deepsize::DeepSizeOf;
use docopt::Docopt;
use ext_sort::ExternalChunk;
use lazy_static::lazy_static;
use numfmt::{Formatter, Numeric, Precision};
use rand::RngCore;
use rand_chacha::ChaCha8Rng;
use rand_seeder::Seeder;
use regex::{Captures, Regex};
use serde::de::DeserializeOwned;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Delimiter};
use crate::dates;
use crate::select::SelectColumns;
use crate::CliResult;

pub fn version() -> String {
    let (maj, min, pat, pre) = (
        option_env!("CARGO_PKG_VERSION_MAJOR"),
        option_env!("CARGO_PKG_VERSION_MINOR"),
        option_env!("CARGO_PKG_VERSION_PATCH"),
        option_env!("CARGO_PKG_VERSION_PRE"),
    );
    match (maj, min, pat, pre) {
        (Some(maj), Some(min), Some(pat), Some(pre)) => {
            if pre.is_empty() {
                format!("{}.{}.{}", maj, min, pat)
            } else {
                format!("{}.{}.{}-{}", maj, min, pat, pre)
            }
        }
        _ => "".to_owned(),
    }
}

lazy_static! {
    static ref FLAG_REGEX: Regex = Regex::new(r"([\s,/\(])(--?[A-Za-z][\w\-]*)").unwrap();
    static ref SECTION_REGEX: Regex = Regex::new("(?im)^.*(?:usage|options?):|---+").unwrap();
    static ref DIMMED_REGEX: Regex =
        Regex::new(r"\[--\]|\[?<[\w|\-]+>(?:\.{3})?\]?|\[[\w\s:§|]+\]|\s+[\$>][^\n]+|\*[^*\n]+\*")
            .unwrap();
    static ref QUOTE_REGEX: Regex = Regex::new(r#"(?m)"[^"\n]+"|'[^'\n]+'|`[^`\n]+`"#).unwrap();
    static ref MAIN_SECTION_REGEX: Regex = Regex::new("(?m)^#+.+").unwrap();
    static ref MAIN_COMMAND_REGEX: Regex = Regex::new(r"(?m)^\s{4}[\w\-]+").unwrap();
    static ref MAIN_ALIAS_REGEX: Regex = Regex::new(r"\([^\)\s]+\)").unwrap();
    static ref URL_REGEX: Regex = Regex::new(r"https?://\S+").unwrap();
}

pub fn colorize_help(help: &str) -> String {
    let help = FLAG_REGEX.replace_all(help, |caps: &Captures| {
        caps[1].to_string() + &caps[2].cyan().to_string()
    });
    let help = MAIN_SECTION_REGEX
        .replace_all(&help, |caps: &Captures| caps[0].yellow().bold().to_string());
    let help =
        SECTION_REGEX.replace_all(&help, |caps: &Captures| caps[0].yellow().bold().to_string());
    let help = QUOTE_REGEX.replace_all(&help, |caps: &Captures| caps[0].green().to_string());

    let help = DIMMED_REGEX.replace_all(&help, |caps: &Captures| {
        caps[0].dimmed().white().to_string()
    });

    let help = URL_REGEX.replace_all(&help, |caps: &Captures| caps[0].blue().to_string());

    help.into_owned()
}

pub fn colorize_main_help(help: &str) -> String {
    let help =
        MAIN_SECTION_REGEX.replace_all(help, |caps: &Captures| caps[0].yellow().bold().to_string());
    let help = MAIN_COMMAND_REGEX.replace_all(&help, |caps: &Captures| {
        "    ".to_string() + &caps[0][4..].cyan().bold().to_string()
    });
    let help = MAIN_ALIAS_REGEX.replace_all(&help, |caps: &Captures| caps[0].dimmed().to_string());

    help.replace("xan", &"xan".red().to_string())
}

pub fn get_args<T>(usage: &str, argv: &[&str]) -> CliResult<T>
where
    T: DeserializeOwned,
{
    Docopt::new(usage)
        .and_then(|d| {
            d.argv(argv.iter().copied())
                .version(Some(version()))
                .help(true)
                .deserialize()
        })
        .map_err(From::from)
}

pub fn many_configs(
    inps: &[String],
    delim: Option<Delimiter>,
    no_headers: bool,
    select: Option<&SelectColumns>,
) -> Result<Vec<Config>, String> {
    let mut inps = inps.to_vec();
    if inps.is_empty() {
        inps.push("-".to_owned()); // stdin
    }
    let confs = inps
        .into_iter()
        .map(|p| {
            let mut conf = Config::new(&Some(p))
                .delimiter(delim)
                .no_headers(no_headers);

            if let Some(sel) = select {
                conf = conf.select(sel.clone());
            }

            conf
        })
        .collect::<Vec<_>>();
    errif_greater_one_stdin(&confs)?;
    Ok(confs)
}

pub fn errif_greater_one_stdin(inps: &[Config]) -> Result<(), String> {
    let nstd = inps.iter().filter(|inp| inp.is_std()).count();
    if nstd > 1 {
        return Err("At most one <stdin> input is allowed.".to_owned());
    }
    Ok(())
}

pub type Idx = Option<usize>;

pub fn range(start: Idx, end: Idx, len: Idx, index: Idx) -> Result<(usize, usize), String> {
    match (start, end, len, index) {
        (None, None, None, Some(i)) => Ok((i, i + 1)),
        (_, _, _, Some(_)) => Err("--index cannot be used with --start, --end or --len".to_owned()),
        (_, Some(_), Some(_), None) => {
            Err("--end and --len cannot be used at the same time.".to_owned())
        }
        (_, None, None, None) => Ok((start.unwrap_or(0), usize::MAX)),
        (_, Some(e), None, None) => {
            let s = start.unwrap_or(0);
            if s > e {
                Err(format!(
                    "The end of the range ({}) must be greater than or\n\
                             equal to the start of the range ({}).",
                    e, s
                ))
            } else {
                Ok((s, e))
            }
        }
        (_, None, Some(l), None) => {
            let s = start.unwrap_or(0);
            Ok((s, s + l))
        }
    }
}

/// Create a directory recursively, avoiding the race conditons fixed by
/// https://github.com/rust-lang/rust/pull/39799.
fn create_dir_all_threadsafe(path: &Path) -> io::Result<()> {
    // Try 20 times. This shouldn't theoretically need to be any larger
    // than the number of nested directories we need to create.
    for _ in 0..20 {
        match fs::create_dir_all(path) {
            // This happens if a directory in `path` doesn't exist when we
            // test for it, and another thread creates it before we can.
            Err(ref err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            other => return other,
        }
        // We probably don't need to sleep at all, because the intermediate
        // directory is already created.  But let's attempt to back off a
        // bit and let the other thread finish.
        thread::sleep(time::Duration::from_millis(25));
    }
    // Try one last time, returning whatever happens.
    fs::create_dir_all(path)
}

/// Represents a filename template of the form `"{}.csv"`, where `"{}"` is
/// the splace to insert the part of the filename generated by `xan`.
#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct FilenameTemplate {
    prefix: String,
    suffix: String,
}

impl FilenameTemplate {
    /// Generate a new filename using `unique_value` to replace the `"{}"`
    /// in the template.
    pub fn filename(&self, unique_value: &str) -> String {
        format!("{}{}{}", &self.prefix, unique_value, &self.suffix)
    }

    /// Create a new, writable file in directory `path` with a filename
    /// using `unique_value` to replace the `"{}"` in the template.  Note
    /// that we do not output headers; the caller must do that if
    /// desired.
    pub fn writer<P>(
        &self,
        path: P,
        unique_value: &str,
    ) -> io::Result<csv::Writer<Box<dyn io::Write + 'static>>>
    where
        P: AsRef<Path>,
    {
        let filename = self.filename(unique_value);
        let full_path = path.as_ref().join(filename);
        if let Some(parent) = full_path.parent() {
            // We may be called concurrently, especially by parallel `xan
            // split`, so be careful to avoid the `create_dir_all` race
            // condition.
            create_dir_all_threadsafe(parent)?;
        }
        let spath = Some(full_path.display().to_string());
        Config::new(&spath).writer_with_options(
            fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true),
        )
    }
}

impl TryFrom<String> for FilenameTemplate {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let chunks = value.split("{}").collect::<Vec<_>>();
        if chunks.len() == 2 {
            Ok(FilenameTemplate {
                prefix: chunks[0].to_owned(),
                suffix: chunks[1].to_owned(),
            })
        } else {
            Err("The --filename argument must contain one '{}'.")
        }
    }
}

pub fn acquire_rng(seed: Option<usize>) -> Box<dyn RngCore> {
    match seed {
        None => Box::new(rand::rng()),
        Some(seed) => Box::new(Seeder::from(seed).into_rng::<ChaCha8Rng>()),
    }
}

pub fn acquire_stty_size() -> Option<termsize::Size> {
    if let Ok(output) = Command::new("/bin/sh")
        .arg("-c")
        .arg("stty size < /dev/tty")
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let parts = text.split_whitespace().take(2).collect::<Vec<_>>();

        if parts.len() < 2 {
            return None;
        }

        let cols: u16 = if let Ok(c) = parts[1].parse() {
            c
        } else {
            return None;
        };

        let rows: u16 = if let Ok(r) = parts[0].parse() {
            r
        } else {
            return None;
        };

        return Some(termsize::Size { cols, rows });
    }

    None
}

pub fn acquire_term_cols(cols_override: &Option<usize>) -> usize {
    match cols_override {
        None => match termsize::get() {
            None => match acquire_stty_size() {
                None => 80,
                Some(size) => size.cols as usize,
            },
            Some(size) => size.cols as usize,
        },
        Some(c) => *c,
    }
}

pub fn acquire_term_rows(rows_override: &Option<usize>) -> usize {
    match rows_override {
        None => match termsize::get() {
            None => match acquire_stty_size() {
                None => 30,
                Some(size) => size.rows as usize,
            },
            Some(size) => size.rows as usize,
        },
        Some(c) => *c,
    }
}

pub fn acquire_term_cols_ratio(cols_override: &Option<String>) -> Result<usize, &str> {
    let mut cols = acquire_term_cols(&None);

    if let Some(spec) = cols_override {
        if spec.contains('.') {
            let ratio = spec.parse::<f64>().map_err(|_| "--cols is invalid!")?;

            cols = (cols as f64 * ratio).trunc().abs() as usize;
        } else {
            cols = spec.parse::<usize>().map_err(|_| "--cols is invalid!")?;
        }
    }

    Ok(cols)
}

pub fn acquire_term_rows_ratio(rows_override: &Option<String>) -> Result<usize, &str> {
    let mut rows = acquire_term_rows(&None);

    if let Some(spec) = rows_override {
        if spec.contains('.') {
            let ratio = spec.parse::<f64>().map_err(|_| "--rows is invalid!")?;

            rows = (rows as f64 * ratio).trunc().abs() as usize;
        } else {
            rows = spec.parse::<usize>().map_err(|_| "--rows is invalid!")?;
        }
    }

    Ok(rows)
}

thread_local! {
    static NUMBER_FORMATTER: RefCell<numfmt::Formatter> = RefCell::new(
        Formatter::new()
            .precision(Precision::Significance(5))
            .separator(',')
            .unwrap()
    );
}

pub fn format_number_with_formatter<T: Numeric>(formatter: &mut numfmt::Formatter, x: T) -> String {
    let mut string = formatter.fmt2(x).to_string();

    if let Some(i) = string.find('.') {
        if string[i + 1..].chars().all(|c| c == '0') {
            string.truncate(i);
        }
    }

    string
}

pub fn format_number<T: Numeric>(x: T) -> String {
    NUMBER_FORMATTER.with_borrow_mut(|f| format_number_with_formatter(f, x))
}

pub fn could_be_url(string: &str) -> bool {
    if string.starts_with("http://") || string.starts_with("https://") {
        return !string.contains(' ');
    }

    false
}

#[derive(PartialEq, Debug)]
pub enum ColorOrStyles {
    Color(Color),
    Styles(Styles),
}

pub fn colorizer_by_type(string: &str) -> ColorOrStyles {
    match string {
        "true" | "TRUE" | "True" | "false" | "FALSE" | "False" | "yes" | "no" => {
            return ColorOrStyles::Color(Color::Cyan)
        }
        "NULL" | "null" | "na" | "NA" | "None" | "n/a" | "N/A" | "nan" | "NaN" | "<empty>"
        | "<null>" | "<rest>" | "." | "-" => return ColorOrStyles::Styles(Styles::Dimmed),
        _ => (),
    };

    match string.trim_start().parse::<f64>() {
        Ok(_) => ColorOrStyles::Color(Color::Red),
        Err(_) => {
            if could_be_url(string) {
                ColorOrStyles::Color(Color::Blue)
            } else if dates::could_be_date(string) {
                ColorOrStyles::Color(Color::Magenta)
            } else {
                ColorOrStyles::Color(Color::Green)
            }
        }
    }
}

pub fn colorizer_by_rainbow(index: usize, string: &str) -> ColorOrStyles {
    if string == "<empty>" {
        return ColorOrStyles::Styles(Styles::Dimmed);
    }

    let index = index % 7;

    match index {
        0 => ColorOrStyles::Color(Color::Red),
        1 => ColorOrStyles::Color(Color::Green),
        2 => ColorOrStyles::Color(Color::Yellow),
        3 => ColorOrStyles::Color(Color::Blue),
        4 => ColorOrStyles::Color(Color::Magenta),
        5 => ColorOrStyles::Color(Color::Cyan),
        6 => ColorOrStyles::Color(Color::BrightBlack),
        _ => unreachable!(),
    }
}

pub fn colorize(color_or_style: &ColorOrStyles, string: &str) -> ColoredString {
    match color_or_style {
        ColorOrStyles::Color(color) => string.color(*color),
        ColorOrStyles::Styles(styles) => match styles {
            Styles::Dimmed => string.dimmed(),
            _ => unimplemented!(),
        },
    }
}

pub fn highlight_trimmable_whitespace(string: &str) -> String {
    let start = string.len() - string.trim_start().len();
    let end = string.trim_end().len();

    format!(
        "{}{}{}",
        "·".repeat((0..start).len()).white().dimmed(),
        &string[start..end],
        "·".repeat((end..string.len()).len()).white().dimmed()
    )
}

lazy_static! {
    static ref WHITESPACE_REPLACER: Regex = Regex::new(r"\r\n|\n\r|[\n\r\t\f]").unwrap();
}

pub fn sanitize_text_for_multi_line_printing(string: &str) -> String {
    // Soft-hyphens
    let mut string = string.replace('\u{00ad}', "");

    // Control characters
    string.retain(|c| c > '\x1f' || c.is_ascii_whitespace());

    string
}

pub fn sanitize_text_for_single_line_printing(string: &str) -> String {
    let sanitized = sanitize_text_for_multi_line_printing(string);

    match WHITESPACE_REPLACER.replace_all(&sanitized, " ") {
        Cow::Borrowed(_) => sanitized,
        Cow::Owned(s) => s,
    }
}

pub fn unicode_aware_ellipsis(string: &str, max_width: usize) -> String {
    let mut width: usize = 0;
    let graphemes = string.graphemes(true).collect::<Vec<_>>();
    let graphemes_count = graphemes.len();

    let mut take: usize = 0;

    for grapheme in graphemes.iter() {
        width += grapheme.width();

        if width <= max_width {
            take += 1;
            continue;
        }

        break;
    }

    let mut parts = graphemes.into_iter().take(take).collect::<Vec<&str>>();

    if graphemes_count > parts.len() {
        parts.pop();

        let mut elided_width = parts.iter().map(|part| part.width()).sum::<usize>() + 1;

        while elided_width < max_width {
            parts.push(" ");
            elided_width += 1;
        }

        parts.push("…");
    }

    parts.into_iter().collect::<String>()
}

pub fn unicode_aware_pad<'a>(
    left: bool,
    string: &'a str,
    width: usize,
    padding: &str,
    actual_string_width: Option<usize>,
) -> Cow<'a, str> {
    let string_width = actual_string_width.unwrap_or_else(|| string.width());

    if string_width >= width {
        return Cow::Borrowed(string);
    }

    let mut padded = String::with_capacity(width);

    if left {
        for _ in 0..(width - string_width) {
            padded.push_str(padding);
        }

        padded.push_str(string);
    } else {
        padded.push_str(string);

        for _ in 0..(width - string_width) {
            padded.push_str(padding);
        }
    }

    Cow::Owned(padded)
}

pub fn unicode_aware_rpad<'a>(string: &'a str, width: usize, padding: &str) -> Cow<'a, str> {
    unicode_aware_pad(false, string, width, padding, None)
}

// NOTE: adapted from https://docs.rs/is-rtl/0.1.1/src/is_rtl/lib.rs.html#1-30
fn is_rtl(c: char) -> bool {
    matches!(c,
        '\u{600}'..='\u{6FF}'
        | '\u{10840}'..='\u{1085F}'
        | '\u{591}'..='\u{5F4}'
        | '\u{103A0}'..='\u{103D5}'
        | '\u{700}'..='\u{74F}'
    )
}

fn has_rtl(string: &str) -> bool {
    string.chars().any(is_rtl)
}

pub fn unicode_aware_pad_with_ellipsis(
    left: bool,
    string: &str,
    width: usize,
    padding: &str,
) -> String {
    let mut string = unicode_aware_pad(
        left,
        &unicode_aware_ellipsis(string, width),
        width,
        padding,
        None,
    )
    .into_owned();

    // NOTE: we force back to LTR at the end of the string, so it does not destroy
    // table formatting & wrapping.
    if has_rtl(&string) {
        string.push('\u{200E}');
    }

    string
}

pub fn unicode_aware_highlighted_pad_with_ellipsis(
    left: bool,
    string: &str,
    width: usize,
    padding: &str,
    highlight: bool,
) -> String {
    let mut string = unicode_aware_pad(
        left,
        &(if highlight {
            highlight_trimmable_whitespace(&unicode_aware_ellipsis(string, width))
        } else {
            unicode_aware_ellipsis(string, width)
        }),
        width,
        padding,
        Some(string.width()),
    )
    .into_owned();

    // NOTE: we force back to LTR at the end of the string, so it does not destroy
    // table formatting & wrapping.
    if has_rtl(&string) {
        string.push('\u{200E}');
    }

    string
}

pub fn unicode_aware_rpad_with_ellipsis(string: &str, width: usize, padding: &str) -> String {
    unicode_aware_pad_with_ellipsis(false, string, width, padding)
}

pub fn unicode_aware_lpad_with_ellipsis(string: &str, width: usize, padding: &str) -> String {
    unicode_aware_pad_with_ellipsis(true, string, width, padding)
}

pub fn wrap(string: &str, max_width: usize, indent: usize) -> String {
    let indent = " ".repeat(indent);
    let options = textwrap::Options::new(max_width).subsequent_indent(&indent);

    textwrap::fill(string, &options)
}

pub struct EmojiSanitizer {
    pattern: Regex,
}

impl EmojiSanitizer {
    pub fn new() -> Self {
        let mut pattern = String::new();
        pattern.push_str("(?:");

        let mut all_emojis = emojis::iter().collect::<Vec<_>>();
        all_emojis.sort_by_key(|e| std::cmp::Reverse((e.as_bytes().len(), e.as_bytes())));

        for emoji in all_emojis {
            pattern.push_str(&regex::escape(emoji.as_str()));
            pattern.push('|');
        }

        pattern.pop();
        pattern.push(')');

        let pattern = Regex::new(&pattern).unwrap();

        EmojiSanitizer { pattern }
    }

    pub fn sanitize(&self, string: &str) -> String {
        self.pattern
            .replace_all(string, |caps: &Captures| {
                format!(
                    ":{}:",
                    match emojis::get(&caps[0]) {
                        None => "unknown_emoji",
                        Some(emoji) => match emoji.shortcode() {
                            None => "unknown_emoji",
                            Some(shortcode) => shortcode,
                        },
                    }
                )
            })
            .to_string()
    }
}

pub trait ImmutableRecordHelpers<'a> {
    type Cell;

    #[must_use]
    fn replace_at(&self, column_index: usize, new_value: Self::Cell) -> Self;

    #[must_use]
    fn prepend(&self, cell_value: Self::Cell) -> Self;

    #[must_use]
    fn append(&self, cell_value: Self::Cell) -> Self;

    #[must_use]
    fn remove(&self, column_index: usize) -> Self;
}

impl<'a> ImmutableRecordHelpers<'a> for csv::ByteRecord {
    type Cell = &'a [u8];

    fn replace_at(&self, column_index: usize, new_value: Self::Cell) -> Self {
        self.iter()
            .enumerate()
            .map(|(i, v)| if i == column_index { new_value } else { v })
            .collect()
    }

    fn prepend(&self, cell_value: Self::Cell) -> Self {
        let mut new_record = csv::ByteRecord::new();
        new_record.push_field(cell_value);
        new_record.extend(self);

        new_record
    }

    fn append(&self, cell_value: Self::Cell) -> Self {
        let mut new_record = self.clone();
        new_record.push_field(cell_value);
        new_record
    }

    fn remove(&self, column_index: usize) -> Self {
        self.iter()
            .enumerate()
            .filter_map(|(i, c)| if i == column_index { None } else { Some(c) })
            .collect()
    }
}

impl<'a> ImmutableRecordHelpers<'a> for csv::StringRecord {
    type Cell = &'a str;

    fn replace_at(&self, column_index: usize, new_value: Self::Cell) -> Self {
        self.iter()
            .enumerate()
            .map(|(i, v)| if i == column_index { new_value } else { v })
            .collect()
    }

    fn prepend(&self, cell_value: Self::Cell) -> Self {
        let mut new_record = csv::StringRecord::new();
        new_record.push_field(cell_value);
        new_record.extend(self);

        new_record
    }

    fn append(&self, cell_value: Self::Cell) -> Self {
        let mut new_record = self.clone();
        new_record.push_field(cell_value);
        new_record
    }

    fn remove(&self, column_index: usize) -> Self {
        self.iter()
            .enumerate()
            .filter_map(|(i, c)| if i == column_index { None } else { Some(c) })
            .collect()
    }
}

pub fn str_to_csv_byte_record(target: &str) -> csv::ByteRecord {
    let cursor = io::Cursor::new(target);
    let reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(cursor);

    match reader.into_byte_records().next() {
        Some(record) => record.unwrap(),
        None => csv::ByteRecord::new(),
    }
}

pub struct Chunks<I> {
    size: NonZeroUsize,
    inner: I,
}

impl<I> Iterator for Chunks<I>
where
    I: Iterator,
{
    type Item = Vec<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut chunk: Vec<I::Item> = Vec::new();

        while chunk.len() < self.size.get() {
            match self.inner.next() {
                None => {
                    if chunk.is_empty() {
                        return None;
                    }

                    return Some(chunk);
                }
                Some(item) => {
                    chunk.push(item);
                }
            }
        }

        Some(chunk)
    }
}

pub trait ChunksIteratorExt: Sized {
    fn chunks(self, size: NonZeroUsize) -> Chunks<Self>;
}

impl<T: Iterator> ChunksIteratorExt for T {
    fn chunks(self, size: NonZeroUsize) -> Chunks<Self> {
        Chunks { size, inner: self }
    }
}

pub trait JoinIteratorExt {
    fn join(self, sep: &str) -> String;
}

impl<T: Deref<Target = str>, I: Iterator<Item = T>> JoinIteratorExt for I {
    fn join(self, sep: &str) -> String {
        let mut string = String::with_capacity(self.size_hint().0.saturating_sub(1));
        let mut started = false;

        for item in self {
            if started {
                string.push_str(sep);
            } else {
                started = true;
            }

            string.push_str(item.deref());
        }

        string
    }
}

// A custom implementation de/serializing ext-sort chunks as CSV
pub struct DeepSizedByteRecord(pub csv::ByteRecord);

impl DeepSizedByteRecord {
    pub fn as_ref(&self) -> &csv::ByteRecord {
        &self.0
    }

    pub fn into_inner(self) -> csv::ByteRecord {
        self.0
    }
}

impl DeepSizeOf for DeepSizedByteRecord {
    fn deep_size_of(&self) -> usize {
        // Good enough approximation...
        self.0.as_slice().len() + (self.0.len().saturating_sub(1))
    }

    fn deep_size_of_children(&self, _context: &mut deepsize::Context) -> usize {
        self.deep_size_of()
    }
}

pub struct CsvExternalChunk {
    reader: csv::Reader<io::Take<io::BufReader<fs::File>>>,
}

impl ExternalChunk<DeepSizedByteRecord> for CsvExternalChunk {
    type SerializationError = csv::Error;
    type DeserializationError = csv::Error;

    fn new(reader: io::Take<io::BufReader<fs::File>>) -> Self {
        CsvExternalChunk {
            reader: csv::ReaderBuilder::new()
                .has_headers(false)
                .from_reader(reader),
        }
    }

    fn dump(
        chunk_writer: &mut io::BufWriter<fs::File>,
        items: impl IntoIterator<Item = DeepSizedByteRecord>,
    ) -> Result<(), Self::SerializationError> {
        let mut csv_writer = csv::Writer::from_writer(chunk_writer);

        for item in items.into_iter() {
            csv_writer.write_record(item.as_ref())?;
        }

        Ok(())
    }
}

impl Iterator for CsvExternalChunk {
    type Item = Result<
        DeepSizedByteRecord,
        <Self as ExternalChunk<DeepSizedByteRecord>>::DeserializationError,
    >;

    fn next(&mut self) -> Option<Self::Item> {
        let mut record = csv::ByteRecord::new();

        match self.reader.read_byte_record(&mut record) {
            Ok(read) => {
                if read {
                    Some(Ok(DeepSizedByteRecord(record)))
                } else {
                    None
                }
            }
            Err(err) => Some(Err(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unicode_aware_ellipsis() {
        assert_eq!(unicode_aware_ellipsis("abcde", 10), "abcde".to_string());
        assert_eq!(unicode_aware_ellipsis("abcde", 5), "abcde".to_string());
        assert_eq!(unicode_aware_ellipsis("abcde", 4), "abc…".to_string());
        assert_eq!(unicode_aware_ellipsis("abcde", 3), "ab…".to_string());
    }

    #[test]
    fn test_emoji_sanitizer() {
        let sanitizer = EmojiSanitizer::new();

        assert_eq!(
            sanitizer.sanitize("👩 hello 👩‍👩‍👧‍👦"),
            ":woman: hello :family_woman_woman_girl_boy:"
        );
    }

    macro_rules! brec {
        () => {
            csv::ByteRecord::new()
        };
        ( $( $x:expr ),* ) => {
            {
                let mut record = csv::ByteRecord::new();
                $(
                    record.push_field($x);
                )*
                record
            }
        };
    }

    #[test]
    fn test_str_to_csv_byte_record() {
        assert_eq!(str_to_csv_byte_record(""), brec![]);
        assert_eq!(str_to_csv_byte_record("test"), brec![b"test"]);
        assert_eq!(str_to_csv_byte_record("test,ok"), brec![b"test", b"ok"]);
        assert_eq!(
            str_to_csv_byte_record("\"test, ok\",\"\"\"John"),
            brec![b"test, ok", b"\"John"]
        );
    }

    #[test]
    fn test_join_iterator_ext() {
        let strings = ["a", "b", "c"];

        assert_eq!(std::iter::empty::<&str>().join("|"), String::from(""));
        assert_eq!(strings.iter().cloned().join("|"), String::from("a|b|c"));
    }
}
