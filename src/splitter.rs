use std::io::{BufRead, BufReader, Read, Result};

use memchr::{memchr, memchr2};

#[derive(Debug)]
enum SplitRecordResult {
    InputEmpty,
    Record,
    End,
}

#[derive(Debug)]
enum RecordSplitterState {
    InUnquoted,
    InQuoted,
    InQuote,
}

struct RecordSplitter {
    quote: u8,
    state: RecordSplitterState,
    was_empty: bool,
}

impl RecordSplitter {
    fn new(quote: u8) -> Self {
        Self {
            quote,
            state: RecordSplitterState::InUnquoted,
            was_empty: false,
        }
    }

    fn split_record(&mut self, input: &[u8]) -> (SplitRecordResult, usize) {
        use RecordSplitterState::*;

        if input.is_empty() {
            if !self.was_empty {
                self.was_empty = true;
                return (SplitRecordResult::Record, 0);
            }

            return (SplitRecordResult::End, 0);
        }

        let mut pos: usize = 0;

        while pos < input.len() {
            match self.state {
                InUnquoted => {
                    // Here we are moving to next quote or end of line
                    if let Some(offset) = memchr2(b'\n', self.quote, &input[pos..]) {
                        pos += offset;

                        let byte = input[pos];

                        pos += 1;

                        if byte == b'\n' {
                            return (SplitRecordResult::Record, pos);
                        }

                        // Here, c is guaranteed to be a quote
                        self.state = InQuoted;
                    } else {
                        break;
                    }
                }
                InQuoted => {
                    // Here we moving to next quote
                    if let Some(offset) = memchr(self.quote, &input[pos..]) {
                        pos += offset + 1;
                        self.state = InQuote;
                    } else {
                        break;
                    }
                }
                InQuote => {
                    let byte = input[pos];

                    if byte == self.quote {
                        self.state = InQuoted;
                    } else {
                        self.state = InUnquoted;
                    }

                    pos += 1;
                }
            }
        }

        (SplitRecordResult::InputEmpty, input.len())
    }
}

pub fn count_records<R: Read>(reader: R, quote: u8) -> Result<u64> {
    use SplitRecordResult::*;

    let mut count: u64 = 0;
    let mut bufreader = BufReader::new(reader);
    let mut splitter = RecordSplitter::new(quote);

    loop {
        let input = bufreader.fill_buf()?;

        let (result, pos) = splitter.split_record(&input);

        bufreader.consume(pos);

        match result {
            End => break,
            InputEmpty => continue,
            Record => {
                count += 1;
            }
        };
    }

    Ok(count)
}

// TEST: empty fields, empty lines, empty input, clrf, invalid quoted parse, test stopping parser right on quote,
// don't count empty lines (need to trim)

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_count() {
        let mut reader = Cursor::new("name\njohn\nlucy");

        assert_eq!(count_records(&mut reader, b'"').unwrap(), 3);
    }
}
