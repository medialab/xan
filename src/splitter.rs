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

        self.record_was_read = false;

        let mut pos: usize = 0;

        while pos < input.len() {
            match self.state {
                Unquoted => {
                    // Skipping empty lines
                    if input[pos] == b'\n' {
                        pos += 1;
                        self.record_was_read = true;
                        continue;
                    }

                    // Here we are moving to next quote or end of line
                    if let Some(offset) = memchr2(b'\n', self.quote, &input[pos..]) {
                        pos += offset;

                        let byte = input[pos];

                        pos += 1;

                        if byte == b'\n' {
                            self.record_was_read = true;
                            return (SplitRecordResult::Record, pos);
                        }

                        // Here, c is guaranteed to be a quote
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

pub fn count_records<R: Read>(reader: R, capacity: usize, quote: u8) -> Result<u64> {
    use SplitRecordResult::*;

    let mut count: u64 = 0;
    let mut bufreader = BufReader::with_capacity(capacity, reader);
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
// TODO: test quote lol and invalid last row?
// TODO: emit record, trimmed, with mmap
// TODO: BOM

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_count() {
        // Empty
        assert_eq!(count_records(&mut Cursor::new(""), 1024, b'"').unwrap(), 0);

        // Single cells with various empty lines
        let tests = vec![
            "name\njohn\nlucy",
            "name\njohn\nlucy\n",
            "name\n\njohn\r\nlucy\n",
            "name\n\njohn\r\nlucy\n\n",
            "\nname\njohn\nlucy",
        ];

        for test in tests {
            for capacity in [32usize, 2, 1] {
                let mut reader = Cursor::new(test);

                assert_eq!(
                    count_records(&mut reader, capacity, b'"').unwrap(),
                    3,
                    "capacity={} string={:?}",
                    capacity,
                    test
                );
            }
        }

        // Multiple cells
        let mut reader = Cursor::new("name,surname,age\njohn,landy,45\nlucy,rose,67");
        assert_eq!(count_records(&mut reader, 1024, b'"').unwrap(), 3);

        // Quoting
        let mut reader = Cursor::new("name,surname,age\n\"john\",\"landy, the \"\"everlasting\"\" bastard\",45\nlucy,rose,67");
        assert_eq!(count_records(&mut reader, 1024, b'"').unwrap(), 3);
    }
}
