//! Native PalmDB/MOBI reader. Supports legacy PalmDOC records and EXTH metadata.
//!
//! This is deliberately NOT a full KF8/AZW3 renderer: true KF8 books use a
//! different HTML/resource model and must be rejected rather than misrendered.

use super::{make_cfi, strip_xml_tags, BookChapter, BookFormat, BookMetadata, BookParagraph, NativeBookDocument};
use std::path::Path;

const MAX_TEXT_BYTES: usize = 64 * 1024 * 1024;

/// Palm database signature of MOBI containers.
pub fn is_mobi_or_azw3_pdb(bytes: &[u8]) -> bool {
    bytes.len() >= 78 && &bytes[60..68] == b"BOOKMOBI"
}

fn read_be_u32(data: &[u8], pos: usize) -> Option<usize> {
    data.get(pos..pos.checked_add(4)?)?
        .try_into()
        .ok()
        .map(|raw: [u8; 4]| u32::from_be_bytes(raw) as usize)
}

fn decode_palmdoc(input: &[u8], remaining_budget: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        i += 1;
        match b {
            0 => out.push(0),
            1..=8 => {
                let n = b as usize;
                let chunk = input.get(i..i + n).ok_or("truncated PalmDOC literal run")?;
                out.extend_from_slice(chunk);
                i += n;
            }
            9..=0x7f => out.push(b),
            0x80..=0xbf => {
                let next = *input.get(i).ok_or("truncated PalmDOC back reference")?;
                i += 1;
                let pair = ((u16::from(b) << 8) | u16::from(next)) & 0x3fff;
                let distance = (pair >> 3) as usize;
                let length = ((pair & 7) as usize) + 3;
                if distance == 0 || distance > out.len() {
                    return Err("invalid PalmDOC back reference".to_owned());
                }
                for _ in 0..length {
                    let value = out[out.len() - distance];
                    out.push(value);
                }
            }
            0xc0..=0xff => {
                out.push(b' ');
                out.push(b ^ 0x80);
            }
        }
        if out.len() > remaining_budget {
            return Err("MOBI text exceeds 64 MiB safety limit".to_owned());
        }
    }
    Ok(out)
}

/// Reads a classic MOBI/PalmDOC container. A genuine KF8/AZW3 record is
/// rejected until its separate text and resource formats are supported.
pub fn parse_mobi_or_azw3_bytes(
    path: &Path,
    relative_path: &str,
    bytes: &[u8],
    format_hint: BookFormat,
) -> Result<NativeBookDocument, String> {
    if !is_mobi_or_azw3_pdb(bytes) {
        return Err("not a valid BOOKMOBI PalmDB container".to_owned());
    }
    let count = u16::from_be_bytes([bytes[76], bytes[77]]) as usize;
    let header_end = 78 + count.checked_mul(8).ok_or("PalmDB record count overflow")?;
    if count < 2 || header_end > bytes.len() {
        return Err("invalid PalmDB record list".to_owned());
    }
    let mut offsets = Vec::with_capacity(count + 1);
    for n in 0..count {
        let off = read_be_u32(bytes, 78 + n * 8).ok_or("invalid PalmDB record offset")?;
        if off < header_end || off > bytes.len() || offsets.last().is_some_and(|&last| last > off) {
            return Err("PalmDB record offsets are outside file or out of order".to_owned());
        }
        offsets.push(off);
    }
    offsets.push(bytes.len());
    let record0 = &bytes[offsets[0]..offsets[1]];
    if record0.len() < 16 {
        return Err("truncated PalmDOC header".to_owned());
    }
    let compression = u16::from_be_bytes([record0[0], record0[1]]);
    let text_record_count = u16::from_be_bytes([record0[8], record0[9]]) as usize;
    if text_record_count == 0 || text_record_count >= count {
        return Err("MOBI text records are missing or truncated".to_owned());
    }

    let mut title = String::from_utf8_lossy(&bytes[..32])
        .trim_end_matches('\0').trim().to_owned();
    if title.is_empty() {
        title = path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "MOBI Book".to_owned());
    }
    let mut authors = Vec::new();
    let mut language = None;

    if record0.len() >= 132 && &record0[16..20] == b"MOBI" {
        let mobi_len = read_be_u32(record0, 20).ok_or("missing MOBI header length")?;
        let mobi_version = read_be_u32(record0, 36).unwrap_or(0);
        if mobi_version >= 8 {
            return Err("KF8/AZW3 layout is not implemented; use existing book renderer".to_owned());
        }

        let title_start = read_be_u32(record0, 84).unwrap_or(0);
        let title_len = read_be_u32(record0, 88).unwrap_or(0);
        if let Some(full_title) = title_start
            .checked_add(title_len)
            .and_then(|end| record0.get(title_start..end))
        {
            let decoded = String::from_utf8_lossy(full_title).trim().to_owned();
            if !decoded.is_empty() {
                title = decoded;
            }
        }

        let exth_start = 16usize.saturating_add(mobi_len);
        let has_exth = read_be_u32(record0, 128).unwrap_or(0) & 0x40 != 0;
        if has_exth && record0.get(exth_start..exth_start.saturating_add(4)) == Some(&b"EXTH"[..]) {
            let entries = read_be_u32(record0, exth_start + 8).unwrap_or(0).min(1024);
            let mut cur = exth_start + 12;
            for _ in 0..entries {
                let Some(kind) = read_be_u32(record0, cur) else { break };
                let Some(len) = read_be_u32(record0, cur + 4) else { break };
                if len < 8 { break; }
                let Some(payload) = cur.checked_add(len).and_then(|end| record0.get(cur + 8..end)) else {
                    break;
                };
                let val = String::from_utf8_lossy(payload).trim().to_owned();
                if !val.is_empty() {
                    match kind {
                        100 => authors.push(val),
                        503 => title = val,
                        524 => language = Some(val),
                        _ => {}
                    }
                }
                cur += len;
            }
        }
    }

    let mut raw_text = Vec::new();
    for n in 1..=text_record_count {
        let record = &bytes[offsets[n]..offsets[n + 1]];
        let remaining = MAX_TEXT_BYTES.saturating_sub(raw_text.len());
        let decoded = match compression {
            1 => {
                if record.len() > remaining {
                    return Err("MOBI text exceeds 64 MiB safety limit".to_owned());
                }
                record.to_vec()
            }
            2 => decode_palmdoc(record, remaining)?,
            other => return Err(format!("unsupported PalmDOC compression {other}")),
        };
        raw_text.extend_from_slice(&decoded);
    }
    let html = String::from_utf8_lossy(&raw_text);
    let mut chapters = Vec::new();
    let mut total_paragraphs = 0;
    for chunk in html.split("<mbp:pagebreak/>") {
        let mut chapter_title = format!("Глава {}", chapters.len() + 1);
        for tag in ["h1", "h2"] {
            let open = format!("<{tag}>");
            let close = format!("</{tag}>");
            if let Some(start) = chunk.find(&open) {
                if let Some(end) = chunk[start + open.len()..].find(&close) {
                    let value = strip_xml_tags(&chunk[start + open.len()..start + open.len() + end]);
                    if !value.trim().is_empty() {
                        chapter_title = value.trim().to_owned();
                        break;
                    }
                }
            }
        }
        let mut paragraphs = Vec::new();
        let mut cursor = 0;
        while let Some(start_rel) = chunk[cursor..].find("<p>") {
            let begin = cursor + start_rel + 3;
            let Some(end_rel) = chunk[begin..].find("</p>") else { break };
            let text = strip_xml_tags(&chunk[begin..begin + end_rel])
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if !text.is_empty() {
                let index = paragraphs.len();
                paragraphs.push(BookParagraph {
                    index,
                    text,
                    is_heading: false,
                    cfi: make_cfi(chapters.len(), index),
                });
                total_paragraphs += 1;
            }
            cursor = begin + end_rel + 4;
        }
        if !paragraphs.is_empty() {
            chapters.push(BookChapter {
                index: chapters.len(),
                title: chapter_title,
                paragraphs,
            });
        }
    }
    if chapters.is_empty() {
        return Err("MOBI contains no readable paragraphs".to_owned());
    }

    Ok(NativeBookDocument {
        path: path.to_path_buf(),
        relative_path: relative_path.to_owned(),
        format: format_hint,
        metadata: BookMetadata { title, authors, language },
        chapters,
        total_paragraphs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(html: &[u8], compression: u16, version: u32) -> Vec<u8> {
        let mut record0 = vec![0u8; 148];
        record0[0..2].copy_from_slice(&compression.to_be_bytes());
        record0[8..10].copy_from_slice(&1u16.to_be_bytes());
        record0[16..20].copy_from_slice(b"MOBI");
        record0[20..24].copy_from_slice(&132u32.to_be_bytes());
        record0[36..40].copy_from_slice(&version.to_be_bytes());
        let mut out = vec![0u8; 94];
        out[..4].copy_from_slice(b"Book");
        out[60..68].copy_from_slice(b"BOOKMOBI");
        out[76..78].copy_from_slice(&2u16.to_be_bytes());
        out[78..82].copy_from_slice(&94u32.to_be_bytes());
        out[86..90].copy_from_slice(&(94u32 + record0.len() as u32).to_be_bytes());
        out.extend_from_slice(&record0);
        out.extend_from_slice(html);
        out
    }

    #[test]
    fn native_mobi_uncompressed_chapters_and_cfi() {
        let bytes = fixture(
            b"<html><h1>First</h1><p>One</p><mbp:pagebreak/><h1>Second</h1><p>Two</p></html>",
            1,
            6,
        );
        let book = parse_mobi_or_azw3_bytes(
            Path::new("example.mobi"), "Files/example.mobi", &bytes, BookFormat::Mobi,
        ).unwrap();
        assert_eq!(book.metadata.title, "Book");
        assert_eq!(book.total_paragraphs, 2);
        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[1].paragraphs[0].cfi, make_cfi(1, 0));
        assert_eq!(book.chapters[1].paragraphs[0].text, "Two");
    }

    #[test]
    fn native_mobi_palmdoc_compressed_record() {
        // ASCII outside PalmDOC control ranges can be stored verbatim.
        let bytes = fixture(b"<p>Hello world</p>", 2, 6);
        let book = parse_mobi_or_azw3_bytes(
            Path::new("compressed.mobi"), "Files/compressed.mobi", &bytes, BookFormat::Mobi,
        ).unwrap();
        assert_eq!(book.total_paragraphs, 1);
        assert_eq!(book.chapters[0].paragraphs[0].text, "Hello world");
    }

    #[test]
    fn native_mobi_rejects_corrupt_offsets_and_kf8() {
        let mut data = fixture(b"<p>One</p>", 1, 6);
        data[86..90].copy_from_slice(&999_999u32.to_be_bytes());
        assert!(parse_mobi_or_azw3_bytes(Path::new("broken.mobi"), "b.mobi", &data, BookFormat::Mobi).is_err());
        let kf8 = fixture(b"<p>One</p>", 1, 8);
        let msg = parse_mobi_or_azw3_bytes(Path::new("book.azw3"), "book.azw3", &kf8, BookFormat::Azw3).unwrap_err();
        assert!(msg.contains("KF8"));
    }

    #[test]
    fn native_mobi_palmdoc_literal_and_invalid_reference() {
        let output = decode_palmdoc(&[4, 0xf0, 0x9f, 0x98, 0x80], 20);
        assert!(output.is_ok());
        let bad = decode_palmdoc(&[0x80, 0x80], 20);
        assert!(bad.is_err());
        assert!(decode_palmdoc(b"abcdef", 2).is_err());
    }
}
