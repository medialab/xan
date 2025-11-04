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
use memmap2::Mmap;
use regex::bytes::Regex;

use crate::read;
use crate::select::{SelectedColumns, Selection};
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
    Ndjson,
    Vcf,
    Gtf,
    Sam,
    Bed,
}

impl TabularDataKind {
    fn has_headers(&self) -> bool {
        !matches!(self, Self::Ndjson | Self::Gtf | Self::Sam | Self::Bed)
    }

    fn has_no_quoting(&self) -> bool {
        matches!(self, Self::Cdx | Self::Ndjson)
    }

    fn header_pattern(&self) -> Option<Regex> {
        match self {
            Self::Vcf => Some(Regex::new("^#CHROM\t").unwrap()),
            Self::Gtf => Some(Regex::new("^#").unwrap()),
            Self::Sam => Some(Regex::new("^@").unwrap()),
            Self::Bed => Some(Regex::new("^(?:track|browser|#)").unwrap()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Compression {
    Gzip,
    Zstd,
}

impl Compression {
    fn infer_from_path(path: &str) -> Option<Self> {
        if path.ends_with(".gz") {
            Some(Self::Gzip)
        } else if path.ends_with(".zst") {
            Some(Self::Zstd)
        } else {
            None
        }
    }

    fn strip_ext<'p>(&self, path: &'p str) -> &'p str {
        let suffix = match self {
            Self::Gzip => ".gz",
            Self::Zstd => ".zst",
        };

        path.strip_suffix(suffix).unwrap_or(path)
    }
}

pub trait SeekRead: Seek + Read {}
impl<T: Seek + Read> SeekRead for T {}

type PairResult = CliResult<(String, Option<String>)>;

#[derive(Debug)]
pub struct Config {
    pub path: Option<PathBuf>, // None implies <stdin>
    select_columns: Option<SelectedColumns>,
    pub delimiter: u8,
    pub no_headers: bool,
    flexible: bool,
    terminator: csv::Terminator,
    pub quote: u8,
    quote_style: csv::QuoteStyle,
    double_quote: bool,
    escape: Option<u8>,
    comment: Option<u8>,
    trim: bool,
    quoting: bool,
    compression: Option<Compression>,
    tabular_data_kind: TabularDataKind,
}

impl Config {
    pub fn has_known_extension(path: &str) -> bool {
        let uncompressed_path = path.strip_suffix(".gz").unwrap_or(path);

        [
            ".csv", ".tsv", ".tab", ".ssv", ".scsv", ".psv", ".cdx", ".ndjson", ".jsonl", ".vcf",
            ".gtf", ".gff2", ".bed", ".sam", ".bed",
        ]
        .iter()
        .any(|ext| uncompressed_path.ends_with(ext))
    }

    pub fn is_chunkable(path: &str) -> bool {
        if !Self::has_known_extension(path) {
            return false;
        }

        if path.ends_with(".gz") {
            Self::new(&Some(path.to_string())).is_indexed_gzip()
        } else {
            true
        }
    }

    pub fn new(path: &Option<String>) -> Self {
        let (path, delimiter, compression, tabular_data_kind) = match *path {
            None => (None, b',', None, TabularDataKind::RegularCsv),
            Some(ref s) if s.deref() == "-" => (None, b',', None, TabularDataKind::RegularCsv),
            Some(ref s) => {
                let compression = Compression::infer_from_path(s);
                let raw_s = compression.map(|c| c.strip_ext(s)).unwrap_or(s);
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
                } else if raw_s.ends_with(".ndjson") || raw_s.ends_with(".jsonl") {
                    kind = TabularDataKind::Ndjson;
                    b'\t'
                } else if raw_s.ends_with(".vcf") {
                    kind = TabularDataKind::Vcf;
                    b'\t'
                } else if raw_s.ends_with(".gtf") || raw_s.ends_with(".gff2") {
                    kind = TabularDataKind::Gtf;
                    b'\t'
                } else if raw_s.ends_with(".sam") {
                    kind = TabularDataKind::Sam;
                    b'\t'
                } else if raw_s.ends_with(".bed") {
                    kind = TabularDataKind::Bed;
                    b'\t'
                } else {
                    b','
                };

                (Some(PathBuf::from(s)), delim, compression, kind)
            }
        };

        Self {
            path,
            select_columns: None,
            delimiter,
            no_headers: !tabular_data_kind.has_headers(),
            flexible: false,
            terminator: csv::Terminator::Any(b'\n'),
            quote: if tabular_data_kind.has_no_quoting() {
                b'\x00'
            } else {
                b'"'
            },
            quote_style: csv::QuoteStyle::Necessary,
            double_quote: true,
            escape: None,
            comment: None,
            quoting: true,
            trim: false,
            compression,
            tabular_data_kind,
        }
    }

    pub fn with_pretend_path(path: &Option<String>, pretend: Option<&str>) -> Self {
        match pretend {
            None => Self::new(path),
            Some(dummy) => {
                let mut conf = Self::new(&Some(dummy.to_string()));
                conf.path = path.as_ref().map(PathBuf::from);
                conf
            }
        }
    }

    pub fn set_compression(&mut self, compression: Compression) {
        self.compression = Some(compression);
    }

    pub fn is_compressed(&self) -> bool {
        self.compression.is_some()
    }

    pub fn std() -> Config {
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

        if !self.tabular_data_kind.has_headers() {
            yes = true;
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

    pub fn comment(mut self, comment: Option<u8>) -> Config {
        self.comment = comment;
        self
    }

    pub fn trim(mut self, yes: bool) -> Config {
        self.trim = yes;
        self
    }

    pub fn quoting(mut self, yes: bool) -> Config {
        self.quoting = yes;
        self
    }

    pub fn select(mut self, sel_cols: SelectedColumns) -> Config {
        self.select_columns = Some(sel_cols);
        self
    }

    pub fn is_std(&self) -> bool {
        self.path.is_none()
    }

    pub fn mmap(&self) -> io::Result<Option<Mmap>> {
        if self.is_std() || self.is_compressed() {
            return Ok(None);
        }

        let file = fs::File::open(self.path.as_ref().unwrap())?;

        let map = unsafe { Mmap::map(&file)? };

        #[cfg(unix)]
        {
            map.advise(memmap2::Advice::Sequential)?;
            map.advise(memmap2::Advice::WillNeed)?;
        }

        Ok(Some(map))
    }

    pub fn selection<'a, H>(&self, first_record: H) -> Result<Selection, String>
    where
        H: IntoIterator<Item = &'a [u8]>,
    {
        match self.select_columns {
            None => Err("Config has no 'SelectColums'. Did you call \
                         Config::select?"
                .to_owned()),
            Some(ref sel) => sel.selection(first_record, !self.no_headers),
        }
    }

    pub fn single_selection<'a, H>(&self, first_record: H) -> Result<usize, String>
    where
        H: IntoIterator<Item = &'a [u8]>,
    {
        match self.select_columns {
            None => Err("Config has no 'SelectColums'. Did you call \
                         Config::select?"
                .to_owned()),
            Some(ref sel) => sel.single_selection(first_record, !self.no_headers),
        }
    }

    pub fn writer(&self) -> io::Result<csv::Writer<Box<dyn io::Write + Send + 'static>>> {
        Ok(self.csv_writer_from_writer(self.io_writer()?))
    }

    pub fn simd_writer(&self) -> io::Result<simd_csv::Writer<Box<dyn io::Write + Send + 'static>>> {
        Ok(self.simd_csv_writer_from_writer(self.io_writer()?))
    }

    pub fn simd_writer_with_options(
        &self,
        options: &fs::OpenOptions,
    ) -> io::Result<simd_csv::Writer<Box<dyn io::Write + 'static>>> {
        Ok(self.simd_csv_writer_from_writer(self.io_writer_with_options(options)?))
    }

    fn process_typical_headers(
        &self,
        mut reader: Box<dyn Read + Send + 'static>,
    ) -> CliResult<Box<dyn Read + Send + 'static>> {
        match self.tabular_data_kind {
            TabularDataKind::Cdx => {
                if !read::consume_cdx_header(&mut reader)? {
                    Err("invalid CDX header!")?;
                }

                Ok(reader)
            }
            TabularDataKind::Vcf => {
                if let Some((_, fixed_reader)) = read::consume_header_until(
                    reader,
                    &self.tabular_data_kind.header_pattern().unwrap(),
                )? {
                    Ok(Box::new(fixed_reader))
                } else {
                    Err(CliError::from("invalid VCF header!"))
                }
            }
            TabularDataKind::Gtf | TabularDataKind::Sam | TabularDataKind::Bed => {
                if let Some((_, fixed_reader)) = read::consume_header_while(
                    reader,
                    &self.tabular_data_kind.header_pattern().unwrap(),
                )? {
                    Ok(Box::new(fixed_reader))
                } else {
                    Err(CliError::from("invalid header!"))
                }
            }
            _ => Ok(reader),
        }
    }

    fn process_typical_headers_seek<R: Read + Seek>(&self, reader: &mut R) -> CliResult<()> {
        match self.tabular_data_kind {
            TabularDataKind::Cdx => {
                if !read::consume_cdx_header(reader)? {
                    Err("invalid CDX header!")?;
                }
            }
            TabularDataKind::Vcf => {
                let (pos, fixed_reader) = read::consume_header_until(
                    reader,
                    &self.tabular_data_kind.header_pattern().unwrap(),
                )?
                .ok_or("invalid VCF header!")?;

                fixed_reader.into_inner().1.seek(SeekFrom::Start(pos))?;
            }
            TabularDataKind::Gtf | TabularDataKind::Sam | TabularDataKind::Bed => {
                let (pos, fixed_reader) = read::consume_header_while(
                    reader,
                    &self.tabular_data_kind.header_pattern().unwrap(),
                )?
                .ok_or("invalid header!")?;

                fixed_reader.into_inner().1.seek(SeekFrom::Start(pos))?;
            }
            _ => (),
        };

        Ok(())
    }

    pub fn reader(&self) -> CliResult<csv::Reader<Box<dyn io::Read + Send + 'static>>> {
        Ok(self.csv_reader_from_reader(self.io_reader()?))
    }

    pub fn simd_reader(&self) -> CliResult<simd_csv::Reader<Box<dyn io::Read + Send + 'static>>> {
        Ok(self.simd_csv_reader_from_reader(self.io_reader()?))
    }

    pub fn simd_zero_copy_reader(
        &self,
    ) -> CliResult<simd_csv::ZeroCopyReader<Box<dyn io::Read + Send + 'static>>> {
        Ok(self.simd_zero_copy_csv_reader_from_reader(self.io_reader()?))
    }

    pub fn simd_splitter(
        &self,
    ) -> CliResult<simd_csv::Splitter<Box<dyn io::Read + Send + 'static>>> {
        Ok(self.simd_csv_splitter_from_reader(self.io_reader()?))
    }

    pub fn simd_seeker(
        &self,
    ) -> CliResult<Option<simd_csv::Seeker<Box<dyn SeekRead + Send + 'static>>>> {
        Ok(self.simd_csv_seeker_from_reader(self.io_reader_for_random_access()?)?)
    }

    pub fn io_reader(&self) -> CliResult<Box<dyn io::Read + Send + 'static>> {
        Ok(match self.path {
            None => {
                if io::stdin().is_terminal() {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "failed to read CSV data from stdin. Did you forget to give a path to your file?"))?;
                } else {
                    let x = io::stdin();

                    let reader: Box<dyn Read + Send + 'static> =
                        if let Some(compression) = self.compression {
                            match compression {
                                Compression::Gzip => Box::new(MultiGzDecoder::new(x)),
                                Compression::Zstd => Box::new(zstd::Decoder::new(x)?),
                            }
                        } else {
                            Box::new(x)
                        };

                    self.process_typical_headers(reader)?
                }
            }
            Some(ref p) => match fs::File::open(p) {
                Ok(x) => {
                    let reader: Box<dyn Read + Send + 'static> =
                        if let Some(compression) = self.compression {
                            match compression {
                                Compression::Gzip => Box::new(MultiGzDecoder::new(x)),
                                Compression::Zstd => Box::new(zstd::Decoder::new(x)?),
                            }
                        } else {
                            Box::new(x)
                        };

                    self.process_typical_headers(reader)?
                }
                Err(err) => {
                    let msg = format!("failed to open {}: {}", p.display(), err);
                    return Err(io::Error::new(io::ErrorKind::NotFound, msg))?;
                }
            },
        })
    }

    pub fn io_reader_for_random_access(&self) -> CliResult<Box<dyn SeekRead + Send + 'static>> {
        let msg = "can't use provided input because it does not allow for random access (e.g. stdin or piping)".to_string();

        match self.path {
            None => Err(io::Error::new(io::ErrorKind::Unsupported, msg))?,
            Some(ref p) => match fs::File::open(p) {
                Ok(mut x) => {
                    if let Some(Compression::Gzip) = self.compression {
                        let index_path_str = p.to_string_lossy() + ".gzi";
                        let index_path = Path::new(index_path_str.as_ref());

                        if index_path.is_file() {
                            let reader = BGZFReader::new(x)?;
                            let index = BGZFIndex::from_reader(fs::File::open(index_path)?)?;
                            let mut indexed_reader = IndexedBGZFReader::new(reader, index)?;

                            self.process_typical_headers_seek(&mut indexed_reader)?;

                            return Ok(Box::new(indexed_reader));
                        }
                    } else {
                        self.process_typical_headers_seek(&mut x)?;
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

    pub fn lines(
        &self,
        select: &Option<SelectedColumns>,
    ) -> CliResult<Box<dyn Iterator<Item = CliResult<String>>>> {
        if let Some(sel) = select {
            let mut csv_reader = self.simd_reader()?;
            let headers = csv_reader.byte_headers()?;
            let column_index = sel.single_selection(headers, !self.no_headers)?;

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
        select: (&Option<SelectedColumns>, &Option<SelectedColumns>),
    ) -> CliResult<Box<dyn Iterator<Item = PairResult>>> {
        if let Some(first_sel) = &select.0 {
            let mut csv_reader = self.simd_reader()?;
            let headers = csv_reader.byte_headers()?;
            let first_column_index = first_sel.single_selection(headers, !self.no_headers)?;
            let second_column_index_opt = select
                .1
                .as_ref()
                .map(|sel| sel.single_selection(headers, !self.no_headers))
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
                if matches!(self.compression, Some(Compression::Gzip)) {
                    let index_path_str = p.to_string_lossy() + ".gzi";
                    let index_path = Path::new(index_path_str.as_ref());

                    index_path.is_file()
                } else {
                    false
                }
            }
        }
    }

    pub fn reverse_reader(
        &self,
    ) -> CliResult<simd_csv::ReverseReader<Box<dyn SeekRead + Send + 'static>>> {
        let io_reader = self.io_reader_for_random_access()?;
        let builder = self.simd_csv_reader_builder();

        Ok(builder.reverse_from_reader(io_reader)?)
    }

    pub fn csv_reader_builder(&self) -> csv::ReaderBuilder {
        let mut builder = csv::ReaderBuilder::new();

        builder
            .flexible(self.flexible)
            .delimiter(self.delimiter)
            .has_headers(!self.no_headers)
            .quote(self.quote)
            .quoting(self.quoting)
            .comment(self.comment)
            .trim(if self.trim {
                csv::Trim::All
            } else {
                csv::Trim::None
            })
            .escape(self.escape);

        builder
    }

    pub fn simd_csv_reader_builder(&self) -> simd_csv::ReaderBuilder {
        let mut builder = simd_csv::ReaderBuilder::new();

        builder
            .delimiter(self.delimiter)
            .quote(self.quote)
            .has_headers(!self.no_headers);

        builder
    }

    pub fn simd_csv_reader_from_reader<R: Read>(&self, rdr: R) -> simd_csv::Reader<R> {
        self.simd_csv_reader_builder().from_reader(rdr)
    }

    pub fn simd_zero_copy_csv_reader_from_reader<R: Read>(
        &self,
        rdr: R,
    ) -> simd_csv::ZeroCopyReader<R> {
        simd_csv::ZeroCopyReaderBuilder::new()
            .delimiter(self.delimiter)
            .quote(self.quote)
            .has_headers(!self.no_headers)
            .from_reader(rdr)
    }

    pub fn simd_csv_splitter_from_reader<R: Read>(&self, rdr: R) -> simd_csv::Splitter<R> {
        simd_csv::SplitterBuilder::new()
            .delimiter(self.delimiter)
            .quote(self.quote)
            .has_headers(!self.no_headers)
            .from_reader(rdr)
    }

    pub fn simd_csv_seeker_from_reader<R: Read + Seek>(
        &self,
        rdr: R,
    ) -> simd_csv::Result<Option<simd_csv::Seeker<R>>> {
        simd_csv::SeekerBuilder::new()
            .delimiter(self.delimiter)
            .quote(self.quote)
            .has_headers(!self.no_headers)
            .from_reader(rdr)
    }

    pub fn csv_reader_from_reader<R: Read>(&self, rdr: R) -> csv::Reader<R> {
        self.csv_reader_builder().from_reader(rdr)
    }

    fn io_writer_with_options(
        &self,
        options: &fs::OpenOptions,
    ) -> io::Result<Box<dyn io::Write + Send + 'static>> {
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

    pub fn buf_io_writer_with_options(
        &self,
        options: &fs::OpenOptions,
    ) -> io::Result<BufWriter<Box<dyn io::Write + Send + 'static>>> {
        Ok(BufWriter::with_capacity(
            32 * (1 << 10),
            self.io_writer_with_options(options)?,
        ))
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
