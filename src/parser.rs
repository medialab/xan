use std::io::{BufRead, BufReader, Read, Result};

use memchr::{memchr, memchr3};

use crate::wide_split::SimdSplitter;

const LAZY_THRESHOLD: usize = 8;

#[inline(always)]
fn lazy_memchr3(n1: u8, n2: u8, n3: u8, haystack: &[u8]) -> Option<usize> {
    // NOTE: haystack will usually not be empty
    if let Some(i) = haystack[..LAZY_THRESHOLD.min(haystack.len())]
        .iter()
        .copied()
        .position(|b| b == n1 || b == n2 || b == n3)
    {
        return Some(i);
    }

    if haystack.len() > LAZY_THRESHOLD {
        return memchr3(n1, n2, n3, &haystack[LAZY_THRESHOLD..]).map(|i| i + LAZY_THRESHOLD);
    }

    None
}

#[derive(Debug)]
enum ReadRecordResult {
    InputEmpty,
    Cr,
    Lf,
    Record,
    End,
}

#[derive(Debug)]
enum RecordReaderState {
    Unquoted,
    Quoted,
    Quote,
}

struct RecordReader {
    quote: u8,
    delimiter: u8,
    state: RecordReaderState,
    record_was_read: bool,
    splitter: SimdSplitter,
}

impl RecordReader {
    fn new(quote: u8, delimiter: u8) -> Self {
        Self {
            quote,
            delimiter,
            state: RecordReaderState::Unquoted,
            // Must be true at the beginning to avoid counting one record for empty input
            record_was_read: true,
            splitter: SimdSplitter::new(quote, delimiter, b'\n'),
        }
    }

    fn read_record(
        &mut self,
        input: &[u8],
        seps_offset: usize,
        seps: &mut Vec<usize>,
    ) -> (ReadRecordResult, usize) {
        use RecordReaderState::*;

        if input.is_empty() {
            if !self.record_was_read {
                self.record_was_read = true;
                return (ReadRecordResult::Record, 0);
            }

            return (ReadRecordResult::End, 0);
        }

        if self.record_was_read {
            if input[0] == b'\n' {
                return (ReadRecordResult::Lf, 1);
            } else if input[0] == b'\r' {
                return (ReadRecordResult::Cr, 1);
            }
        }

        self.record_was_read = false;

        let mut pos: usize = 0;

        while pos < input.len() {
            match self.state {
                Unquoted => {
                    // Here we are moving to next quote or end of line
                    let mut last_offset: Option<usize> = None;

                    for offset in self.splitter.split(&input[pos..]) {
                        last_offset = Some(offset);

                        let byte = input[pos + offset];

                        if byte == self.delimiter {
                            seps.push(seps_offset + pos + offset);
                            continue;
                        }

                        if byte == b'\n' {
                            self.record_was_read = true;
                            return (ReadRecordResult::Record, pos + offset + 1);
                        }

                        // Here, `byte` is guaranteed to be a quote
                        self.state = Quoted;
                        break;
                    }

                    if let Some(offset) = last_offset {
                        pos += offset + 1;
                    } else {
                        break;
                    }
                }
                Quoted => {
                    // Here we moving to next quote
                    if let Some(offset) = memchr(self.quote, &input[pos..]) {
                        pos += offset + 1;
                        self.state = Quote;
                    } else {
                        break;
                    }
                }
                Quote => {
                    let byte = input[pos];

                    pos += 1;

                    if byte == self.quote {
                        self.state = Quoted;
                    } else if byte == self.delimiter {
                        seps.push(seps_offset + pos - 1);
                        self.state = Unquoted;
                    } else if byte == b'\n' {
                        self.record_was_read = true;
                        self.state = Unquoted;
                        return (ReadRecordResult::Record, pos);
                    } else {
                        self.state = Unquoted;
                    }
                }
            }
        }

        (ReadRecordResult::InputEmpty, input.len())
    }
}

pub struct BufferedRecordReader<R> {
    buffer: BufReader<R>,
    reader: RecordReader,
    record: Vec<u8>,
    seps: Vec<usize>,
    actual_buffer_position: Option<usize>,
}

impl<R: Read> BufferedRecordReader<R> {
    pub fn with_capacity(reader: R, capacity: usize, quote: u8, delimiter: u8) -> Self {
        Self {
            buffer: BufReader::with_capacity(capacity, reader),
            reader: RecordReader::new(quote, delimiter),
            record: Vec::with_capacity(capacity),
            seps: Vec::with_capacity(32),
            actual_buffer_position: None,
        }
    }

    pub fn read_record(&mut self) -> Result<Option<(&[u8], &[usize])>> {
        use ReadRecordResult::*;

        self.record.clear();
        self.seps.clear();

        if let Some(last_pos) = self.actual_buffer_position.take() {
            self.buffer.consume(last_pos);
        }

        loop {
            let input = self.buffer.fill_buf()?;

            let (result, pos) = self
                .reader
                .read_record(&input, self.record.len(), &mut self.seps);

            match result {
                End => {
                    self.buffer.consume(pos);
                    return Ok(None);
                }
                Cr | Lf => {
                    self.buffer.consume(pos);
                }
                InputEmpty => {
                    self.record.extend(&input[..pos]);
                    self.buffer.consume(pos);
                }
                Record => {
                    if self.record.is_empty() {
                        self.actual_buffer_position = Some(pos);
                        return Ok(Some((&self.buffer.buffer()[..pos], &self.seps)));
                    } else {
                        self.record.extend(&input[..pos]);
                        self.buffer.consume(pos);

                        return Ok(Some((&self.record, &self.seps)));
                    }
                }
            };
        }
    }
}

// TODO: test empty seps, also in splitter, test empty rows with ""
// TODO: decoding a field means trimming quotes left & right and finding doubled quotes to escape

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_basics() {
        // let csv = "name,surname,age\nJohn,Lucy,84\nCarry,Grant,43";
        let csv = "name,surname,age\n\"john\",\"landy, the \"\"everlasting\"\" bastard\",45\nlucy,rose,\"67\"\njermaine,jackson,\"89\"\n\nkarine,loucan,\"52\"\nrose,\"glib\",12\n\"guillaume\",\"plique\",\"42\"\r\n";

        let mut reader = BufferedRecordReader::with_capacity(Cursor::new(csv), 32, b'"', b',');

        while let Some((record, seps)) = reader.read_record().unwrap() {
            dbg!(bstr::BStr::new(record), seps);
            assert!(seps.iter().copied().all(|i| record[i] == b','));

            let mut offset = 0;

            for i in seps.iter().copied() {
                let field = &record[offset..i];
                offset = i + 1;
                dbg!(bstr::BStr::new(field));
            }

            // TODO: must trim end last clrf
            dbg!(bstr::BStr::new(&record[offset..]));

            dbg!();
        }
    }
}
