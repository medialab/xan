#[allow(deprecated, unused_imports)]
use std::ascii::AsciiExt;
use std::borrow::{Borrow, ToOwned};
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::io::{self, prelude::*, BufReader, BufWriter, IsTerminal, Read, SeekFrom};
use std::ops::Deref;
use std::path::{Path, PathBuf};

use bgzip::index::BGZFIndex;
use bgzip::read::{BGZFReader, IndexedBGZFReader};
use flate2::read::MultiGzDecoder;

use crate::read::{self, ReverseRead};
use crate::record::Record;
use crate::select::{SelectColumns, Selection};
use crate::{CliError, CliResult};

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct Delimiter(pub u8);

/// Delimiter represents values that can be passed from the command line that
/// can be used as a field delimiter in CSV data.
///
/// Its purpose is to ensure that the Unicode character given decodes to a
/// valid ASCII character as required by the CSV parser.
impl Delimiter {
    pub fn as_byte(self) -> u8 {
        self.0
    }
}

impl TryFrom<String> for Delimiter {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            r"\t" => Ok(Delimiter(b'\t')),
            s => {
                if s.len() != 1 {
                    let msg = format!(
                        "Could not convert '{}' to a single \
                                       ASCII character.",
                        s
                    );
                    return Err(msg);
                }
                let c = s.chars().next().unwrap();
                if c.is_ascii() {
                    Ok(Delimiter(c as u8))
                } else {
                    let msg = format!(
                        "Could not convert '{}' \
                                       to ASCII delimiter.",
                        c
                    );
                    Err(msg)
                }
            }
        }
    }
}

#[derive(Debug)]
enum TabularDataKind {
    RegularCsv,
    Cdx,
}

impl TabularDataKind {
    fn is_cdx(&self) -> bool {
        matches!(self, Self::Cdx)
    }
}

pub trait SeekRead: Seek + Read {}
impl<T: Seek + Read> SeekRead for T {}

type PairResult = CliResult<(String, Option<String>)>;

#[derive(Debug)]
pub struct Config {
    pub path: Option<PathBuf>, // None implies <stdin>
    select_columns: Option<SelectColumns>,
    delimiter: u8,
    pub no_headers: bool,
    flexible: bool,
    terminator: csv::Terminator,
    quote: u8,
    quote_style: csv::QuoteStyle,
    double_quote: bool,
    escape: Option<u8>,
    quoting: bool,
    compressed: bool, // TODO: can become a compression type if we need to support more schemes than gz
    tabular_data_kind: TabularDataKind,
}

impl Config {
    pub fn new(path: &Option<String>) -> Config {
        let (path, delim, compressed, tabular_data_kind) = match *path {
            None => (None, b',', false, TabularDataKind::RegularCsv),
            Some(ref s) if s.deref() == "-" => (None, b',', false, TabularDataKind::RegularCsv),
            Some(ref s) => {
                let raw_s = s.strip_suffix(".gz").unwrap_or(s);
                let mut kind = TabularDataKind::RegularCsv;

                let delim = if raw_s.ends_with(".tsv") || raw_s.ends_with(".tab") {
                    b'\t'
                } else if raw_s.ends_with(".ssv") || raw_s.ends_with(".scsv") {
                    b';'
                } else if raw_s.ends_with(".psv") {
                    b'|'
                } else if raw_s.ends_with(".cdx") {
                    kind = TabularDataKind::Cdx;
                    b' '
                } else {
                    b','
                };

                (Some(PathBuf::from(s)), delim, s.ends_with(".gz"), kind)
            }
        };

        let mut config = Config {
            path,
            select_columns: None,
            delimiter: delim,
            no_headers: false,
            flexible: false,
            terminator: csv::Terminator::Any(b'\n'),
            quote: b'"',
            quote_style: csv::QuoteStyle::Necessary,
            double_quote: true,
            escape: None,
            quoting: true,
            compressed,
            tabular_data_kind,
        };

        if config.tabular_data_kind.is_cdx() {
            config.quoting = false;
        }

        config
    }

    pub fn stdin() -> Config {
        Self::new(&None)
    }

    pub fn delimiter(mut self, d: Option<Delimiter>) -> Config {
        if let Some(d) = d {
            self.delimiter = d.as_byte();
        }
        self
    }

    pub fn no_headers(mut self, mut yes: bool) -> Config {
        if env::var("XAN_TOGGLE_HEADERS").unwrap_or("0".to_owned()) == "1" {
            yes = !yes;
        }
        self.no_headers = yes;
        self
    }

    pub fn flexible(mut self, yes: bool) -> Config {
        self.flexible = yes;
        self
    }

    pub fn crlf(mut self, yes: bool) -> Config {
        if yes {
            self.terminator = csv::Terminator::CRLF;
        } else {
            self.terminator = csv::Terminator::Any(b'\n');
        }
        self
    }

    pub fn terminator(mut self, term: csv::Terminator) -> Config {
        self.terminator = term;
        self
    }

    pub fn quote(mut self, quote: u8) -> Config {
        self.quote = quote;
        self
    }

    pub fn quote_style(mut self, style: csv::QuoteStyle) -> Config {
        self.quote_style = style;
        self
    }

    pub fn double_quote(mut self, yes: bool) -> Config {
        self.double_quote = yes;
        self
    }

    pub fn escape(mut self, escape: Option<u8>) -> Config {
        self.escape = escape;
        self
    }

    pub fn quoting(mut self, yes: bool) -> Config {
        self.quoting = yes;
        self
    }

    pub fn select(mut self, sel_cols: SelectColumns) -> Config {
        self.select_columns = Some(sel_cols);
        self
    }

    pub fn is_std(&self) -> bool {
        self.path.is_none()
    }

    pub fn selection<R: Record>(&self, first_record: &R) -> Result<Selection, String> {
        match self.select_columns {
            None => Err("Config has no 'SelectColums'. Did you call \
                         Config::select?"
                .to_owned()),
            Some(ref sel) => sel.selection(first_record, !self.no_headers),
        }
    }

    pub fn single_selection(&self, first_record: &csv::ByteRecord) -> Result<usize, String> {
        match self.select_columns {
            None => Err("Config has no 'SelectColums'. Did you call \
                         Config::select?"
                .to_owned()),
            Some(ref sel) => sel.single_selection(first_record, !self.no_headers),
        }
    }

    pub fn write_headers<R: io::Read, W: io::Write>(
        &self,
        r: &mut csv::Reader<R>,
        w: &mut csv::Writer<W>,
    ) -> csv::Result<()> {
        if !self.no_headers {
            let r = r.byte_headers()?;
            if !r.is_empty() {
                w.write_record(r)?;
            }
        }
        Ok(())
    }

    pub fn writer(&self) -> io::Result<csv::Writer<Box<dyn io::Write + Send + 'static>>> {
        Ok(self.csv_writer_from_writer(self.io_writer()?))
    }

    pub fn simd_writer(&self) -> io::Result<simd_csv::Writer<Box<dyn io::Write + Send + 'static>>> {
        Ok(self.simd_csv_writer_from_writer(self.io_writer()?))
    }

    pub fn writer_with_options(
        &self,
        options: &fs::OpenOptions,
    ) -> io::Result<csv::Writer<Box<dyn io::Write + 'static>>> {
        Ok(self.csv_writer_from_writer(self.io_writer_with_options(options)?))
    }

    #[allow(clippy::single_match)]
    fn read_typical_headers<R: Read>(&self, reader: &mut R) -> CliResult<()> {
        match self.tabular_data_kind {
            TabularDataKind::Cdx => {
                if !read::consume_cdx_header(reader)? {
                    Err("invalid CDX header!")?;
                }
            }
            _ => (),
        };

        Ok(())
    }

    pub fn reader(&self) -> CliResult<csv::Reader<Box<dyn io::Read + Send + 'static>>> {
        Ok(self.csv_reader_from_reader(self.io_reader()?))
    }

    pub fn simd_reader(&self) -> CliResult<simd_csv::Reader<Box<dyn io::Read + Send + 'static>>> {
        let mut reader = self.simd_csv_reader_from_reader(self.io_reader()?);

        reader.strip_bom()?;

        Ok(reader)
    }

    pub fn simd_splitter(
        &self,
    ) -> CliResult<simd_csv::Splitter<Box<dyn io::Read + Send + 'static>>> {
        let mut splitter = self.simd_csv_splitter_from_reader(self.io_reader()?);

        splitter.strip_bom()?;

        Ok(splitter)
    }

    pub fn seekable_reader(&self) -> CliResult<csv::Reader<Box<dyn SeekRead + Send + 'static>>> {
        Ok(self.csv_reader_from_reader(self.io_reader_for_random_access()?))
    }

    pub fn io_reader(&self) -> CliResult<Box<dyn io::Read + Send + 'static>> {
        Ok(match self.path {
            None => {
                if io::stdin().is_terminal() {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "failed to read CSV data from stdin. Did you forget to give a path to your file?"))?;
                } else {
                    Box::new(io::stdin())
                }
            }
            Some(ref p) => match fs::File::open(p) {
                Ok(x) => {
                    let mut reader: Box<dyn Read + Send + 'static> = if self.compressed {
                        Box::new(MultiGzDecoder::new(x))
                    } else {
                        Box::new(x)
                    };

                    self.read_typical_headers(&mut reader)?;

                    reader
                }
                Err(err) => {
                    let msg = format!("failed to open {}: {}", p.display(), err);
                    return Err(io::Error::new(io::ErrorKind::NotFound, msg))?;
                }
            },
        })
    }

    pub fn lines(
        &self,
        select: &Option<SelectColumns>,
    ) -> CliResult<Box<dyn Iterator<Item = CliResult<String>>>> {
        if let Some(sel) = select {
            let mut csv_reader = self.simd_reader()?;
            let headers = csv_reader.peek_byte_record(true)?;
            let column_index = sel.single_selection(&headers, !self.no_headers)?;

            return Ok(Box::new(csv_reader.into_byte_records().map(
                move |result| match result {
                    Err(e) => Err(e)?,
                    Ok(record) => {
                        let line = String::from_utf8(record[column_index].to_vec())
                            .expect("could not decode utf8");

                        Ok(line)
                    }
                },
            )));
        }

        let lines_reader = BufReader::new(self.io_reader()?);

        Ok(Box::new(lines_reader.lines().filter_map(
            |result| match result {
                Err(e) => Some(Err(CliError::from(e))),
                Ok(mut line) => {
                    line.truncate(line.trim_end().len());

                    if line.is_empty() {
                        None
                    } else {
                        Some(Ok(line))
                    }
                }
            },
        )))
    }

    pub fn pairs(
        &self,
        select: (&Option<SelectColumns>, &Option<SelectColumns>),
    ) -> CliResult<Box<dyn Iterator<Item = PairResult>>> {
        if let Some(first_sel) = &select.0 {
            let mut csv_reader = self.simd_reader()?;
            let headers = csv_reader.peek_byte_record(true)?;
            let first_column_index = first_sel.single_selection(&headers, !self.no_headers)?;
            let second_column_index_opt = select
                .1
                .as_ref()
                .map(|sel| sel.single_selection(&headers, !self.no_headers))
                .transpose()?;

            return Ok(Box::new(csv_reader.into_byte_records().map(
                move |result| match result {
                    Err(e) => Err(e)?,
                    Ok(record) => {
                        let a = String::from_utf8(record[first_column_index].to_vec())
                            .expect("could not decode utf8");

                        let b = second_column_index_opt.map(|second_column_index| {
                            String::from_utf8(record[second_column_index].to_vec())
                                .expect("could not decode utf8")
                        });

                        Ok((a, b))
                    }
                },
            )));
        }

        let lines_reader = BufReader::new(self.io_reader()?);

        Ok(Box::new(lines_reader.lines().filter_map(
            |result| match result {
                Err(e) => Some(Err(CliError::from(e))),
                Ok(mut line) => {
                    line.truncate(line.trim_end().len());

                    if line.is_empty() {
                        None
                    } else {
                        Some(Ok((line, None)))
                    }
                }
            },
        )))
    }

    pub fn is_indexed_gzip(&self) -> bool {
        match self.path {
            None => false,
            Some(ref p) => {
                if self.compressed {
                    let index_path_str = p.to_string_lossy() + ".gzi";
                    let index_path = Path::new(index_path_str.as_ref());

                    index_path.is_file()
                } else {
                    false
                }
            }
        }
    }

    pub fn io_reader_for_random_access(&self) -> CliResult<Box<dyn SeekRead + Send + 'static>> {
        let msg = "can't use provided input because it does not allow for random access (e.g. stdin or piping)".to_string();

        match self.path {
            None => Err(io::Error::new(io::ErrorKind::Unsupported, msg))?,
            Some(ref p) => match fs::File::open(p) {
                Ok(mut x) => {
                    if self.compressed {
                        let index_path_str = p.to_string_lossy() + ".gzi";
                        let index_path = Path::new(index_path_str.as_ref());

                        if index_path.is_file() {
                            let reader = BGZFReader::new(x)?;
                            let index = BGZFIndex::from_reader(fs::File::open(index_path)?)?;
                            let mut indexed_reader = IndexedBGZFReader::new(reader, index)?;

                            self.read_typical_headers(&mut indexed_reader)?;

                            return Ok(Box::new(indexed_reader));
                        }
                    } else {
                        self.read_typical_headers(&mut x)?;
                    }

                    match x.borrow().stream_position() {
                        Ok(_) => Ok(Box::new(x)),
                        Err(_) => Err(io::Error::new(io::ErrorKind::Unsupported, msg))?,
                    }
                }
                Err(err) => {
                    let msg = format!("failed to open {}: {}", p.display(), err);
                    Err(io::Error::new(io::ErrorKind::NotFound, msg))?
                }
            },
        }
    }

    pub fn io_reader_at_position_with_limit(
        &self,
        position: u64,
        limit: u64,
    ) -> CliResult<Box<dyn Read + Send + 'static>> {
        let mut reader = self.io_reader_for_random_access()?;

        reader.seek(SeekFrom::Start(position))?;

        Ok(Box::new(reader.take(limit)))
    }

    pub fn reverse_reader(
        &self,
    ) -> CliResult<(
        csv::ByteRecord,
        csv::Reader<Box<dyn io::Read + Send + 'static>>,
    )> {
        let mut io_reader = self.io_reader_for_random_access()?;
        let offset_before_csv_parsing = io_reader.stream_position()?;

        let mut forward_reader = self.csv_reader_from_reader(io_reader);
        let headers = forward_reader.byte_headers()?.clone();

        let offset = if self.no_headers {
            offset_before_csv_parsing
        } else {
            offset_before_csv_parsing + forward_reader.position().byte()
        };

        let filesize = forward_reader.get_mut().seek(SeekFrom::End(0))?;

        let reverse_reader = ReverseRead::new(forward_reader.into_inner(), filesize, offset);
        let mut reader_builder = self.csv_reader_builder();
        reader_builder.has_headers(false);

        Ok((
            headers,
            reader_builder.from_reader(Box::new(reverse_reader)),
        ))
    }

    pub fn csv_reader_builder(&self) -> csv::ReaderBuilder {
        let mut builder = csv::ReaderBuilder::new();

        builder
            .flexible(self.flexible)
            .delimiter(self.delimiter)
            .has_headers(!self.no_headers)
            .quote(self.quote)
            .quoting(self.quoting)
            .escape(self.escape);

        builder
    }

    pub fn simd_csv_reader_from_reader<R: Read>(&self, rdr: R) -> simd_csv::Reader<R> {
        simd_csv::ReaderBuilder::new()
            .delimiter(self.delimiter)
            .quote(self.quote)
            .from_reader(rdr)
    }

    pub fn simd_csv_splitter_from_reader<R: Read>(&self, rdr: R) -> simd_csv::Splitter<R> {
        simd_csv::SplitterBuilder::new()
            .delimiter(self.delimiter)
            .quote(self.quote)
            .from_reader(rdr)
    }

    pub fn csv_reader_from_reader<R: Read>(&self, rdr: R) -> csv::Reader<R> {
        self.csv_reader_builder().from_reader(rdr)
    }

    fn io_writer_with_options(
        &self,
        options: &fs::OpenOptions,
    ) -> io::Result<Box<dyn io::Write + 'static>> {
        Ok(match self.path {
            None => Box::new(io::stdout()),
            Some(ref p) => Box::new(options.open(p)?),
        })
    }

    pub fn io_writer(&self) -> io::Result<Box<dyn io::Write + Send + 'static>> {
        Ok(match self.path {
            None => Box::new(io::stdout()),
            Some(ref p) => Box::new(fs::File::create(p)?),
        })
    }

    pub fn buf_io_writer(&self) -> io::Result<BufWriter<Box<dyn io::Write + Send + 'static>>> {
        Ok(BufWriter::with_capacity(32 * (1 << 10), self.io_writer()?))
    }

    pub fn csv_writer_from_writer<W: io::Write>(&self, wtr: W) -> csv::Writer<W> {
        csv::WriterBuilder::new()
            .flexible(self.flexible)
            .delimiter(self.delimiter)
            .terminator(self.terminator)
            .quote(self.quote)
            .quote_style(self.quote_style)
            .double_quote(self.double_quote)
            .escape(self.escape.unwrap_or(b'\\'))
            .buffer_capacity(32 * (1 << 10))
            .from_writer(wtr)
    }

    pub fn simd_csv_writer_from_writer<W: io::Write>(&self, wtr: W) -> simd_csv::Writer<W> {
        simd_csv::WriterBuilder::with_capacity(32 * (1 << 10))
            .delimiter(self.delimiter)
            .quote(self.quote)
            .from_writer(wtr)
    }
}
