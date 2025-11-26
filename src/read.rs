use std::io::{self, Cursor, Read};

use regex::bytes::Regex;

pub fn consume_cdx_header<R: Read>(reader: &mut R) -> io::Result<bool> {
    let mut buf = [0u8; 5];

    reader.read_exact(&mut buf)?;

    Ok(&buf == b" CDX ")
}

type RecombobulatedReader<R> = io::Chain<Cursor<Vec<u8>>, R>;

pub fn consume_header_until<R: Read>(
    reader: R,
    pattern: &Regex,
) -> io::Result<Option<(u64, RecombobulatedReader<R>)>> {
    let mut line_reader = simd_csv::LineReader::from_reader(reader);
    let mut pos = line_reader.position();
    let mut header_opt: Option<Vec<u8>> = None;

    while let Some(line) = line_reader.read_line()? {
        if !pattern.is_match(line) {
            pos = line_reader.position();
            continue;
        }

        header_opt = Some(line.to_vec());

        break;
    }

    if let Some(mut fixed_data) = header_opt {
        let bufreader = line_reader.into_bufreader();
        fixed_data.push(b'\n');
        fixed_data.extend(bufreader.buffer());

        let fixed_reader = Cursor::new(fixed_data).chain(bufreader.into_inner());

        Ok(Some((pos, fixed_reader)))
    } else {
        Ok(None)
    }
}

pub fn consume_header_while<R: Read>(
    reader: R,
    pattern: &Regex,
) -> io::Result<Option<(u64, RecombobulatedReader<R>)>> {
    let mut line_reader = simd_csv::LineReader::from_reader(reader);
    let mut pos = line_reader.position();
    let mut header_opt: Option<Vec<u8>> = None;

    while let Some(line) = line_reader.read_line()? {
        if pattern.is_match(line) {
            pos = line_reader.position();
            continue;
        }

        header_opt = Some(line.to_vec());

        break;
    }

    if let Some(mut fixed_data) = header_opt {
        let bufreader = line_reader.into_bufreader();
        fixed_data.push(b'\n');
        fixed_data.extend(bufreader.buffer());

        let fixed_reader = Cursor::new(fixed_data).chain(bufreader.into_inner());

        Ok(Some((pos, fixed_reader)))
    } else {
        Ok(None)
    }
}

pub fn consume_lines<R: Read>(
    reader: R,
    limit: usize,
) -> io::Result<Option<(u64, RecombobulatedReader<R>)>> {
    let mut line_reader = simd_csv::LineReader::from_reader(reader);
    let mut pos = line_reader.position();
    let mut seen: usize = 0;
    let mut header_opt: Option<Vec<u8>> = None;

    while let Some(line) = line_reader.read_line()? {
        seen += 1;

        if seen <= limit {
            pos = line_reader.position();
            continue;
        }

        header_opt = Some(line.to_vec());

        break;
    }

    if let Some(mut fixed_data) = header_opt {
        let bufreader = line_reader.into_bufreader();
        fixed_data.push(b'\n');
        fixed_data.extend(bufreader.buffer());

        let fixed_reader = Cursor::new(fixed_data).chain(bufreader.into_inner());

        Ok(Some((pos, fixed_reader)))
    } else {
        Ok(None)
    }
}
