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
    pub size: u64,
    pub max_record_size: u64,
    pub mean_record_size: f64,
    pub first_record_offset: u64,
    pub profile: Vec<f64>,
    pub file_len: u64,
    pub eof: bool,
}

impl InitialRecordsSample {
    pub fn field_count(&self) -> usize {
        self.profile.len()
    }

    pub fn approx_count(&self) -> u64 {
        (self.file_len as f64 / self.mean_record_size).ceil() as u64
    }

    pub fn exact_or_approx_count(&self) -> u64 {
        if self.eof {
            self.size
        } else {
            self.approx_count()
        }
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

// BEWARE: this functions seeks to the end of file to assess file len!
pub fn sample_initial_records<R: Read + Seek>(
    reader: &mut Reader<R>,
    max_records_to_read: u64,
) -> Result<Option<InitialRecordsSample>, csv::Error> {
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

    if i == 0 {
        return Ok(None);
    }

    let profile = (0..field_count)
        .map(|j| profiles.iter().map(|p| p[j] as f64).sum::<f64>() / profiles.len() as f64)
        .collect::<Vec<_>>();

    let eof = !reader.read_byte_record(&mut record)?;
    let file_len = reader.get_mut().seek(SeekFrom::End(0))?;

    Ok(Some(InitialRecordsSample {
        size: i,
        max_record_size: max_record_size.unwrap(),
        mean_record_size: welford.mean().unwrap(),
        first_record_offset,
        profile,
        file_len,
        eof,
    }))
}

// BEWARE: this function assess whether the parsed record is too far AFTER the fact.
// We do this because the given record might not have a position yet on first call.
// Note also that this function might catastrophically overscan and should not be
// used by `next_record_info`.
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

#[derive(Debug, Clone, PartialEq)]
pub enum NextRecordOffsetInferrence {
    Start,
    End,
    Fail,
    WasInQuoted(u64, ByteRecord),
    WasInUnquoted(u64, ByteRecord),
}

impl NextRecordOffsetInferrence {
    pub fn failed(&self) -> bool {
        matches!(self, Self::Fail)
    }

    pub fn offset(&self) -> Option<u64> {
        match self {
            Self::WasInQuoted(offset, _) | Self::WasInUnquoted(offset, _) => Some(*offset),
            _ => None,
        }
    }

    pub fn into_record_with_offset(self) -> Option<(ByteRecord, u64)> {
        match self {
            Self::WasInQuoted(offset, record) | Self::WasInUnquoted(offset, record) => {
                Some((record, offset))
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
struct RecordInfo {
    record: ByteRecord,
}

impl RecordInfo {
    fn profile(&self) -> Vec<usize> {
        self.record.iter().map(|cell| cell.len()).collect()
    }

    fn byte_offset(&self) -> u64 {
        self.record.position().unwrap().byte()
    }

    fn into_inner(self) -> ByteRecord {
        self.record
    }
}

// NOTE: reader MUST be clamped beforehand
fn next_record_info<R: Read>(
    reader: &mut Reader<R>,
    expected_field_count: usize,
) -> Result<Option<RecordInfo>, csv::Error> {
    let mut i: usize = 0;
    let mut info: Option<RecordInfo> = None;
    let mut record = ByteRecord::new();
    let mut alignments: Vec<usize> = Vec::new();

    while reader.read_byte_record(&mut record)? {
        if i > 0 {
            alignments.push(record.len());

            if i == 1 {
                info = Some(RecordInfo {
                    record: record.clone(),
                });
            }
        }

        i += 1;
    }

    // NOTE: if we have less than 2 records beyond the first one, it will be hard to
    // make a correct decision
    // NOTE: last record might be unaligned since we artificially clamp the read buffer
    if alignments.len() < 2
        || alignments[..alignments.len() - 1]
            .iter()
            .any(|l| *l != expected_field_count)
    {
        return Ok(None);
    }

    Ok(info)
}

pub fn find_next_record_offset_from_random_position<F, R>(
    reader: &mut Reader<R>,
    reader_builder: F,
    offset: u64,
    sample: &InitialRecordsSample,
    jump: u64,
) -> Result<NextRecordOffsetInferrence, csv::Error>
where
    F: Fn() -> ReaderBuilder,
    R: Read + Seek,
{
    debug_assert!(jump > 0);

    // First we seek to given random position
    let mut pos = Position::new();
    pos.set_byte(offset);
    reader.seek_raw(SeekFrom::Start(offset), pos)?;

    let mut end_byte = sample.max_record_size * jump;

    let mut altered_reader = reader_builder()
        .flexible(true)
        .has_headers(false)
        .from_reader(reader.get_mut().take(end_byte));

    // Reading as potentially unquoted
    let unquoted_next_record_info = next_record_info(&mut altered_reader, sample.field_count())?;

    // Reading as potentially quoted
    let mut pos = Position::new();
    pos.set_byte(offset);
    reader.seek_raw(SeekFrom::Start(offset), pos)?;

    end_byte += 1;

    // TODO: this would not work with custom quote char, beware
    let mut altered_reader = reader_builder()
        .flexible(true)
        .has_headers(false)
        .from_reader(Cursor::new("\"").chain(reader.get_mut()).take(end_byte));

    let quoted_next_record_info = next_record_info(&mut altered_reader, sample.field_count())?;

    Ok(match (unquoted_next_record_info, quoted_next_record_info) {
        (None, None) => NextRecordOffsetInferrence::Fail,
        (Some(info), None) => NextRecordOffsetInferrence::WasInUnquoted(
            offset + info.byte_offset(),
            info.into_inner(),
        ),
        (None, Some(info)) => NextRecordOffsetInferrence::WasInQuoted(
            offset + info.byte_offset() - 1,
            info.into_inner(),
        ),
        (Some(unquoted_info), Some(quoted_info)) => {
            // Sometimes we might fall within a cell whose contents suspiciously yield
            // the same record structure. In this case we rely on cosine similarity over
            // record profiles to make sure we select the correct offset.
            let unquoted_offset = offset + unquoted_info.byte_offset();
            let quoted_offset = offset + quoted_info.byte_offset() - 1;

            if unquoted_offset == quoted_offset {
                NextRecordOffsetInferrence::WasInUnquoted(
                    unquoted_offset,
                    unquoted_info.into_inner(),
                )
            } else {
                let unquoted_cosine = cosine(&sample.profile, &unquoted_info.profile());
                let quoted_cosine = cosine(&sample.profile, &quoted_info.profile());

                if unquoted_cosine > quoted_cosine {
                    NextRecordOffsetInferrence::WasInUnquoted(
                        unquoted_offset,
                        unquoted_info.into_inner(),
                    )
                } else {
                    NextRecordOffsetInferrence::WasInQuoted(quoted_offset, quoted_info.into_inner())
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

pub struct SegmentationOptions {
    chunks: usize,
    init_sample_size: u64,
    jump_sample_size: u64,
}

impl SegmentationOptions {
    pub fn chunks(count: usize) -> Self {
        Self {
            chunks: count,
            init_sample_size: 128,
            jump_sample_size: 8,
        }
    }
}

pub fn segment_csv_file<F, R>(
    reader: &mut Reader<R>,
    reader_builder: F,
    mut options: SegmentationOptions,
) -> Result<Option<Vec<(u64, u64)>>, csv::Error>
where
    F: Fn() -> ReaderBuilder,
    R: Read + Seek,
{
    let sample = match sample_initial_records(reader, options.init_sample_size)? {
        None => return Ok(None),
        Some(s) => s,
    };

    // File is way too short
    if sample.size < options.chunks as u64 {
        return Ok(Some(vec![(sample.first_record_offset, sample.file_len)]));
    }

    // Limiting number of chunks when file is too short
    options.chunks = options
        .chunks
        .min((sample.file_len / (sample.max_record_size * options.jump_sample_size) - 1) as usize)
        .max(1);

    let offsets = segment_file(sample.file_len, options.chunks);
    let mut segments = Vec::with_capacity(offsets.len());

    for offset in offsets {
        if offset == 0 {
            segments.push(NextRecordOffsetInferrence::Start);
        } else {
            let inferred = find_next_record_offset_from_random_position(
                reader,
                &reader_builder,
                offset,
                &sample,
                options.jump_sample_size,
            )?;

            if inferred.failed() {
                return Ok(None);
            }

            segments.push(inferred);
        }
    }

    debug_assert!(segments[0] == NextRecordOffsetInferrence::Start);

    segments.push(NextRecordOffsetInferrence::End);

    let offsets = segments
        .windows(2)
        .map(|window| {
            (
                match window[0] {
                    NextRecordOffsetInferrence::Start => sample.first_record_offset,
                    _ => window[0].offset().unwrap(),
                },
                window[1].offset().unwrap_or(sample.file_len),
            )
        })
        .collect::<Vec<_>>();

    Ok(Some(offsets))
}
