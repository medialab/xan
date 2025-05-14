use std::io::{self, Cursor, Read, Seek, SeekFrom};

use csv::{ByteRecord, Position, Reader, ReaderBuilder};

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

#[derive(Debug)]
pub struct InitialRecordsSample {
    count: u64,
    stats: Option<(u64, f64)>,
    first_record_offset: u64,
    profile: Vec<f64>,
}

impl InitialRecordsSample {
    fn new(
        count: u64,
        max: Option<u64>,
        mean: Option<f64>,
        first_record_offset: u64,
        profile: Vec<f64>,
    ) -> Self {
        Self {
            count,
            stats: max.map(|m| (m, mean.unwrap())),
            first_record_offset,
            profile,
        }
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn mean(&self) -> Option<f64> {
        self.stats.map(|(_, m)| m)
    }

    pub fn max(&self) -> Option<u64> {
        self.stats.map(|(m, _)| m)
    }
}

fn cosine(profile: &[f64], other: &[usize]) -> f64 {
    let mut self_norm = 0.0;
    let mut other_norm = 0.0;
    let mut intersection = 0.0;

    for (a, b) in profile
        .iter()
        .copied()
        .zip(other.iter().copied().map(|i| i as f64))
    {
        self_norm += a * a;
        other_norm += b * b;
        intersection += a * b;
    }

    intersection / (self_norm * other_norm).sqrt()
}

pub fn sample_initial_records<R: Read + Seek>(
    reader: &mut Reader<R>,
    max_records_to_read: u64,
) -> Result<InitialRecordsSample, csv::Error> {
    // NOTE: it is important to make sure headers have been read
    // so that the first record size does not include header bytes.
    let field_count = reader.byte_headers()?.len();

    let mut record = ByteRecord::new();

    let mut i: u64 = 0;
    let mut max_record_size = None;
    let mut welford = Welford::new();
    let mut first_record_offset = 0;
    let mut last_offset = reader.position().byte();
    let mut profiles: Vec<Vec<usize>> = Vec::with_capacity(max_records_to_read as usize);

    while i < max_records_to_read && reader.read_byte_record(&mut record)? {
        if i == 0 {
            first_record_offset = record.position().unwrap().byte();
        }

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

        profiles.push(record.iter().map(|cell| cell.len()).collect());

        i += 1;
        last_offset = record_byte_pos;
    }

    let profile = (0..field_count)
        .map(|j| profiles.iter().map(|p| p[j] as f64).sum::<f64>() / profiles.len() as f64)
        .collect::<Vec<_>>();

    Ok(InitialRecordsSample::new(
        i,
        max_record_size,
        welford.mean(),
        first_record_offset,
        profile,
    ))
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

#[derive(Debug, Clone, Copy)]
enum NextRecordOffsetInferrence {
    Start,
    End,
    Fail,
    WasInQuoted(u64),
    WasInUnquoted(u64),
}

impl NextRecordOffsetInferrence {
    fn failed(&self) -> bool {
        matches!(self, Self::Fail)
    }

    fn offset(&self) -> Option<u64> {
        match self {
            Self::WasInQuoted(offset) | Self::WasInUnquoted(offset) => Some(*offset),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct RecordInfo {
    byte_offset: u64,
    profile: Vec<usize>,
}

fn next_record_info<R: Read>(
    reader: &mut Reader<R>,
    end_byte: u64,
    expected_field_count: usize,
) -> Result<Option<RecordInfo>, csv::Error> {
    let mut i: usize = 0;
    let mut info: Option<RecordInfo> = None;
    let mut record = ByteRecord::new();

    while read_byte_record_up_to(reader, &mut record, Some(end_byte))? {
        if i > 0 {
            if record.len() != expected_field_count {
                return Ok(None);
            }

            if i == 1 {
                info = Some(RecordInfo {
                    byte_offset: record.position().unwrap().byte(),
                    profile: record.iter().map(|cell| cell.len()).collect(),
                });
            }
        }

        i += 1;
    }

    Ok(info)
}

fn infer_next_record_offset_from_random_position<R: Read + Seek>(
    reader: &mut Reader<R>,
    offset: u64,
    max_record_size: u64,
    profile: &[f64],
    expected_field_count: usize,
    sample_size: u64,
) -> Result<NextRecordOffsetInferrence, csv::Error> {
    // First we seek to given random position
    let mut pos = Position::new();
    pos.set_byte(offset);
    reader.seek_raw(SeekFrom::Start(offset), pos)?;

    debug_assert!(sample_size > 0);

    let mut end_byte = offset + max_record_size * sample_size;

    // Reading as potentially unquoted
    let unquoted_next_record_info = next_record_info(reader, end_byte, expected_field_count)?;

    // Reading as potentially quoted
    let mut pos = Position::new();
    pos.set_byte(offset);
    reader.seek_raw(SeekFrom::Start(offset), pos)?;

    // TODO: quote char must be known if different
    let mut altered_reader = ReaderBuilder::new()
        .flexible(true)
        .has_headers(false)
        .from_reader(Cursor::new("\"").chain(reader.get_mut()));

    end_byte = max_record_size * sample_size + 1;

    let quoted_next_record_info =
        next_record_info(&mut altered_reader, end_byte, expected_field_count)?;

    Ok(match (unquoted_next_record_info, quoted_next_record_info) {
        (None, None) => NextRecordOffsetInferrence::Fail,
        (Some(info), None) => NextRecordOffsetInferrence::WasInUnquoted(info.byte_offset),
        (None, Some(info)) => {
            NextRecordOffsetInferrence::WasInQuoted(offset + info.byte_offset - 1)
        }
        (Some(unquoted_info), Some(quoted_info)) => {
            // Sometimes we might fall within a cell whose contents suspiciously yield
            // the same record structure. In this case we rely on cosine similarity over
            // record profiles to make sure we select the correct offset.
            let unquoted_offset = unquoted_info.byte_offset;
            let quoted_offset = offset + quoted_info.byte_offset - 1;

            if unquoted_offset == quoted_offset {
                NextRecordOffsetInferrence::WasInUnquoted(unquoted_offset)
            } else {
                let unquoted_cosine = cosine(profile, &unquoted_info.profile);
                let quoted_cosine = cosine(profile, &quoted_info.profile);

                if unquoted_cosine > quoted_cosine {
                    NextRecordOffsetInferrence::WasInUnquoted(unquoted_offset)
                } else {
                    NextRecordOffsetInferrence::WasInQuoted(quoted_offset)
                }
            }
        }
    })
}

fn segment_file(file_len: u64, chunks: usize) -> Vec<u64> {
    let mut offsets = vec![0];

    if chunks < 2 {
        return offsets;
    }

    for i in 1..chunks {
        offsets.push(((i as f64 / chunks as f64) * file_len as f64).floor() as u64);
    }

    offsets
}

pub fn segment_csv_file<R: Read + Seek>(
    reader: &mut Reader<R>,
    chunks: usize,
    init_sample_size: u64,
    jump_sample_size: u64,
) -> Result<Option<Vec<(u64, u64)>>, csv::Error> {
    let field_count = reader.byte_headers()?.len();

    if field_count == 0 {
        return Ok(None);
    }

    let sample = sample_initial_records(reader, init_sample_size)?;

    let file_len = reader.get_mut().seek(SeekFrom::End(0))?;

    let max_record_size = match sample.max() {
        None => return Ok(None),
        Some(m) => m,
    };

    // TODO: return single offset if some invariant is not met, e.g. when
    // the file is too small typically
    // TODO: also mind cases where the file is empty or too short

    let mut segments = segment_file(file_len, chunks)
        .iter()
        .copied()
        .map(|offset| {
            if offset == 0 {
                Ok(NextRecordOffsetInferrence::Start)
            } else {
                infer_next_record_offset_from_random_position(
                    reader,
                    offset,
                    max_record_size,
                    &sample.profile,
                    field_count,
                    jump_sample_size,
                )
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if segments.iter().any(NextRecordOffsetInferrence::failed) {
        return Ok(None);
    }

    segments.push(NextRecordOffsetInferrence::End);

    let offsets = segments
        .windows(2)
        .map(|window| {
            (
                match window[0] {
                    NextRecordOffsetInferrence::Start => sample.first_record_offset,
                    _ => window[0].offset().unwrap(),
                },
                window[1].offset().unwrap_or(file_len),
            )
        })
        .collect::<Vec<_>>();

    Ok(Some(offsets))
}
