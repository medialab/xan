use std::fs::File;
use std::io::{BufRead, BufReader, Read, Result};
use std::path::Path;

use memchr::{memchr, memchr2};
use memmap2::Mmap;

#[derive(Debug)]
enum SplitRecordResult {
    InputEmpty,
    Cr,
    Lf,
    Record,
    End,
}

#[derive(Debug)]
enum RecordSplitterState {
    Unquoted,
    Quoted,
    Quote,
}

struct RecordSplitter {
    quote: u8,
    state: RecordSplitterState,
    record_was_read: bool,
}

impl RecordSplitter {
    fn new(quote: u8) -> Self {
        Self {
            quote,
            state: RecordSplitterState::Unquoted,
            // Must be true at the beginning to avoid counting one record for empty input
            record_was_read: true,
        }
    }

    fn split_record(&mut self, input: &[u8]) -> (SplitRecordResult, usize) {
        use RecordSplitterState::*;

        if input.is_empty() {
            if !self.record_was_read {
                self.record_was_read = true;
                return (SplitRecordResult::Record, 0);
            }

            return (SplitRecordResult::End, 0);
        }

        if self.record_was_read {
            if input[0] == b'\n' {
                return (SplitRecordResult::Lf, 1);
            } else if input[0] == b'\r' {
                return (SplitRecordResult::Cr, 1);
            }
        }

        self.record_was_read = false;

        let mut pos: usize = 0;

        while pos < input.len() {
            match self.state {
                Unquoted => {
                    // Here we are moving to next quote or end of line
                    if let Some(offset) = memchr2(b'\n', self.quote, &input[pos..]) {
                        pos += offset;

                        let byte = input[pos];

                        pos += 1;

                        if byte == b'\n' {
                            self.record_was_read = true;
                            return (SplitRecordResult::Record, pos);
                        }

                        // Here, `byte` is guaranteed to be a quote
                        self.state = Quoted;
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

                    if byte == self.quote {
                        self.state = Quoted;
                    } else if byte == b'\n' {
                        self.record_was_read = true;
                        self.state = Unquoted;
                        return (SplitRecordResult::Record, pos);
                    } else {
                        self.state = Unquoted;
                    }

                    pos += 1;
                }
            }
        }

        (SplitRecordResult::InputEmpty, input.len())
    }
}

pub struct BufferedRecordSplitter<R> {
    buffer: BufReader<R>,
    splitter: RecordSplitter,
}

impl<R: Read> BufferedRecordSplitter<R> {
    pub fn with_capacity(reader: R, capacity: usize, quote: u8) -> Self {
        Self {
            buffer: BufReader::with_capacity(capacity, reader),
            splitter: RecordSplitter::new(quote),
        }
    }

    pub fn count_records(&mut self) -> Result<u64> {
        use SplitRecordResult::*;

        let mut count: u64 = 0;

        loop {
            let input = self.buffer.fill_buf()?;

            let (result, pos) = self.splitter.split_record(&input);

            self.buffer.consume(pos);

            match result {
                End => break,
                InputEmpty | Cr | Lf => continue,
                Record => {
                    count += 1;
                }
            };
        }

        Ok(count)
    }

    pub fn split_record(&mut self, record: &mut Vec<u8>) -> Result<bool> {
        use SplitRecordResult::*;

        record.clear();

        loop {
            let input = self.buffer.fill_buf()?;

            let (result, pos) = self.splitter.split_record(&input);

            match result {
                End => {
                    self.buffer.consume(pos);
                    return Ok(false);
                }
                Cr | Lf => {
                    self.buffer.consume(pos);
                }
                InputEmpty => {
                    record.extend(&input[..pos]);
                    self.buffer.consume(pos);
                }
                Record => {
                    record.extend(&input[..pos]);
                    self.buffer.consume(pos);
                    break;
                }
            };
        }

        Ok(true)
    }
}

pub struct MmapRecordSplitter {
    // file: File,
    map: Mmap,
    splitter: RecordSplitter,
}

impl MmapRecordSplitter {
    pub fn new<P: AsRef<Path>>(path: P, quote: u8) -> Result<Self> {
        let file = File::open(path)?;

        let map = unsafe { Mmap::map(&file)? };

        Ok(Self {
            // file,
            map,
            splitter: RecordSplitter::new(quote),
        })
    }

    pub fn count_records(&mut self) -> u64 {
        use SplitRecordResult::*;

        let mut i: usize = 0;
        let mut count: u64 = 0;

        loop {
            let (result, pos) = self.splitter.split_record(&self.map[i..]);

            i += pos;

            match result {
                End => break,
                InputEmpty | Cr | Lf => continue,
                Record => {
                    count += 1;
                }
            };
        }

        count
    }
}

// TODO: mmap split_record and test with regex match

// TEST: empty fields, empty lines, empty input, clrf, invalid quoted parse, test stopping parser right on quote,
// don't count empty lines (need to trim)
// TODO: test quote lol and invalid last row?
// TODO: emit record, trimmed, with mmap
// TODO: BOM

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn count_records<R: Read>(rdr: R, capacity: usize) -> u64 {
        let mut splitter = BufferedRecordSplitter::with_capacity(rdr, capacity, b'"');
        splitter.count_records().unwrap()
    }

    #[test]
    fn test_count() {
        // Empty
        assert_eq!(count_records(&mut Cursor::new(""), 1024), 0);

        // Single cells with various empty lines
        let tests = vec![
            "name\njohn\nlucy",
            "name\njohn\nlucy\n",
            "name\n\njohn\r\nlucy\n",
            "name\n\njohn\r\nlucy\n\n",
            "name\n\n\njohn\r\n\r\nlucy\n\n\n",
            "\nname\njohn\nlucy",
            "\n\nname\njohn\nlucy",
            "\r\n\r\nname\njohn\nlucy",
            "name\njohn\nlucy\r\n",
            "name\njohn\nlucy\r\n\r\n",
        ];

        for capacity in [32usize, 4, 3, 2, 1] {
            for test in tests.iter() {
                let mut reader = Cursor::new(test);

                assert_eq!(
                    count_records(&mut reader, capacity),
                    3,
                    "capacity={} string={:?}",
                    capacity,
                    test
                );
            }
        }

        // Multiple cells
        let mut reader = Cursor::new("name,surname,age\njohn,landy,45\nlucy,rose,67");
        assert_eq!(count_records(&mut reader, 1024), 3);

        // Quoting
        for capacity in [1024usize, 32usize, 4, 3, 2, 1] {
            let mut reader = Cursor::new("name,surname,age\n\"john\",\"landy, the \"\"everlasting\"\" bastard\",45\nlucy,rose,\"67\"\njermaine,jackson,\"89\"\n\nkarine,loucan,\"52\"\r\n");

            assert_eq!(
                count_records(&mut reader, capacity),
                5,
                "capacity={}",
                capacity
            );
        }

        // Different separator
        let mut reader = Cursor::new("name\tsurname\tage\njohn\tlandy\t45\nlucy\trose\t67");
        assert_eq!(count_records(&mut reader, 1024), 3);
    }
}
