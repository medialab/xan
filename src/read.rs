use std::io::{self, Read, Seek, SeekFrom};

use csv::{ByteRecord, Reader};

use crate::moonblade::agg::Welford;

pub struct ReverseRead<R> {
    input: R,
    offset: u64,
    ptr: u64,
}

impl<R: Seek + Read> Read for ReverseRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let buff_size = buf.len() as u64;

        if self.ptr == self.offset {
            return Ok(0);
        }

        if self.offset + buff_size > self.ptr {
            let e = (self.ptr - self.offset) as usize;

            self.input.seek(SeekFrom::Start(self.offset))?;
            self.input.read_exact(&mut buf[0..e])?;

            buf[0..e].reverse();

            self.ptr = self.offset;

            Ok(e)
        } else {
            let new_position = self.ptr - buff_size;

            self.input.seek(SeekFrom::Start(new_position))?;
            self.input.read_exact(buf)?;
            buf.reverse();

            self.ptr -= buff_size;

            Ok(buff_size as usize)
        }
    }
}

impl<R: Seek + Read> ReverseRead<R> {
    pub fn new(input: R, filesize: u64, offset: u64) -> Self {
        Self {
            input,
            offset,
            ptr: filesize,
        }
    }
}

pub struct RecordSizesSample {
    count: u64,
    stats: Option<(u64, f64)>,
}

impl RecordSizesSample {
    fn new(count: u64, max: Option<u64>, mean: Option<f64>) -> Self {
        Self {
            count,
            stats: max.map(|m| (m, mean.unwrap())),
        }
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn mean(&self) -> Option<f64> {
        self.stats.map(|(_, m)| m)
    }
}

pub fn sample_record_sizes<R: Read + Seek>(
    reader: &mut Reader<R>,
    max_records_to_read: u64,
) -> Result<RecordSizesSample, csv::Error> {
    // NOTE: it is important to make sure headers have been read
    // so that the first record size does not include header bytes.
    reader.byte_headers()?;

    let mut record = ByteRecord::new();

    let mut i: u64 = 0;
    let mut max_record_size = None;
    let mut welford = Welford::new();
    let mut last_offset = reader.position().byte();

    while i < max_records_to_read && reader.read_byte_record(&mut record)? {
        let record_byte_pos = reader.position().byte();
        let record_size = record_byte_pos - last_offset;

        match &mut max_record_size {
            None => {
                max_record_size = Some(record_size);
            }
            Some(current_size) => {
                if record_size > *current_size {
                    *current_size = record_size;
                }
            }
        }

        welford.add(record_size as f64);

        i += 1;
        last_offset = record_byte_pos;
    }

    Ok(RecordSizesSample::new(i, max_record_size, welford.mean()))
}

pub fn read_byte_record_up_to<R: Read>(
    reader: &mut Reader<R>,
    record: &mut ByteRecord,
    up_to: Option<u64>,
) -> Result<bool, csv::Error> {
    let was_read = reader.read_byte_record(record)?;

    if !was_read {
        return Ok(false);
    }

    if let Some(byte) = up_to {
        if record.position().unwrap().byte() >= byte {
            return Ok(false);
        }
    }

    Ok(true)
}
