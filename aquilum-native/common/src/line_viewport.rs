//! Framework-independent Markdown viewport for large native documents.
//! Stores UTF-8 text once plus one byte offset per logical line.
//! Rendering a viewport borrows only its visible lines, without building a
//! full document of text widgets or copying the entire file on scrolling.
use crate::text_positions::{range_utf16_to_bytes, PositionError};
use std::mem::size_of;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    Reversed,
    OutOfBounds,
    InsideUnicodeScalar,
    InvalidUtf16(PositionError),
}
impl From<PositionError> for EditError {
    fn from(error: PositionError) -> Self { Self::InvalidUtf16(error) }
}

pub struct LineViewport {
    text: String,
    starts: Vec<usize>,
}
impl LineViewport {
    pub fn new(text: String) -> Self {
        let starts = build_starts(&text);
        Self { text, starts }
    }
    pub fn text(&self) -> &str { &self.text }
    pub fn into_text(self) -> String { self.text }
    pub fn bytes(&self) -> usize { self.text.len() }
    pub fn line_count(&self) -> usize { self.starts.len() }

    pub fn byte_of_line(&self, line: usize) -> Option<usize> {
        self.starts.get(line).copied()
    }
    pub fn visible_text(&self, first_line: usize, count: usize) -> &str {
        if count == 0 || first_line >= self.starts.len() { return ""; }
        let start = self.starts[first_line];
        let after_line = first_line.saturating_add(count);
        let end = self.starts.get(after_line).copied().unwrap_or(self.text.len());
        &self.text[start..end]
    }
    /// Allocator capacity only, not whole-process resident RAM or GPU caches.
    pub fn backing_capacity_bytes(&self) -> usize {
        self.text.capacity() + self.starts.capacity() * size_of::<usize>()
    }
    /// Re-index only the affected line boundaries; updates coordinates of
    /// following starts in O(line count) rather than scanning all text.
    pub fn replace_bytes(&mut self, range: Range<usize>, insert: &str) -> Result<(), EditError> {
        if range.start > range.end { return Err(EditError::Reversed); }
        if range.end > self.text.len() { return Err(EditError::OutOfBounds); }
        if !self.text.is_char_boundary(range.start) || !self.text.is_char_boundary(range.end) {
            return Err(EditError::InsideUnicodeScalar);
        }
        let replaced_len = range.end - range.start;
        let added_len = insert.len();
        let begin = self.starts.partition_point(|&p| p <= range.start);
        let stop = self.starts.partition_point(|&p| p <= range.end);
        let new_offsets: Vec<usize> = insert.bytes().enumerate()
            .filter(|&(_, b)| b == b'\n')
            .map(|(i, _)| range.start + i + 1)
            .collect();
        let new_count = new_offsets.len();
        self.text.replace_range(range, insert);
        self.starts.splice(begin..stop, new_offsets);
        for pos in &mut self.starts[begin + new_count..] {
            if added_len >= replaced_len { *pos += added_len - replaced_len; }
            else { *pos -= replaced_len - added_len; }
        }
        debug_assert_eq!(self.starts.first(), Some(&0));
        debug_assert!(self.starts.windows(2).all(|w| w[0] < w[1]));
        Ok(())
    }
    /// Matches CodeMirror/Yrs global UTF-16 offsets; rejects surrogate splits.
    /// A sparse coordinate index will replace the O(document) conversion.
    pub fn replace_utf16(&mut self, start: usize, end: usize, insert: &str)
        -> Result<(), EditError>
    {
        let bytes = range_utf16_to_bytes(&self.text, start, end)?;
        self.replace_bytes(bytes, insert)
    }
}
fn build_starts(text: &str) -> Vec<usize> {
    let mut starts = Vec::with_capacity(text.bytes().filter(|&b| b == b'\n').count() + 1);
    starts.push(0);
    for (at, byte) in text.bytes().enumerate() {
        if byte == b'\n' { starts.push(at + 1); }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(v: &LineViewport) {
        assert_eq!(v.starts, build_starts(v.text()));
        assert_eq!(v.line_count(), v.text().split('\n').count());
        for first in 0..v.line_count() {
            let reference = v.text().split_inclusive('\n')
                .skip(first).take(3).collect::<String>();
            assert_eq!(v.visible_text(first, 3), reference);
        }
    }
    #[test]
    fn empty_and_final_newline_are_valid_lines() {
        for text in ["", "a", "a\n", "\n", "\n\n", "А\n🌍\n"] {
            let v = LineViewport::new(text.to_owned());
            assert_eq!(v.visible_text(0, 0), "");
            assert_eq!(v.visible_text(usize::MAX, 5), "");
            check(&v);
        }
    }
    #[test]
    fn updates_after_insertion_deletion_and_newline_join() {
        let mut v = LineViewport::new("A\nРусский\n🌍\nZ".to_owned());
        v.replace_bytes(0..0, "Header\n").unwrap();
        check(&v);
        let start = v.text().find("Русский").unwrap();
        v.replace_bytes(start..start+14, "T").unwrap();
        check(&v);
        let point = v.text().find("\n🌍").unwrap();
        v.replace_bytes(point..point+1, "").unwrap();
        check(&v);
        assert!(v.replace_bytes(1000..1001, "").is_err());
    }
    #[test]
    fn validates_utf16_surrogates_and_russian_text() {
        let mut v = LineViewport::new("Старт 🌍 конец\r\n".to_owned());
        assert!(v.replace_utf16(7, 8, "X").is_err());
        v.replace_utf16(6, 8, "👍🏼").unwrap();
        assert_eq!(v.text(), "Старт 👍🏼 конец\r\n");
        check(&v);
    }
    #[test]
    fn randomized_unicode_edit_equivalence() {
        let mut baseline = "Заметка 🌍\nРусский\nEnglish\n".repeat(5);
        let mut v = LineViewport::new(baseline.clone());
        let mut seed = 0x7234_5123_u32;
        let inserts = ["a", "\n", "\r\n", "🇷🇺", "👍🏼", "", "таблица", "\n> [!quote]\n"];
        for _ in 0..220 {
            seed ^= seed << 13; seed ^= seed >> 17; seed ^= seed << 5;
            let boundaries: Vec<usize> = baseline.char_indices().map(|(i,_)|i)
                .chain(std::iter::once(baseline.len())).collect();
            let a = (seed as usize) % boundaries.len();
            let b = (a + (seed as usize / 5) % 3).min(boundaries.len()-1);
            let start = boundaries[a];
            let end = boundaries[b];
            let from16 = baseline[..start].encode_utf16().count();
            let to16 = baseline[..end].encode_utf16().count();
            let insert = inserts[seed as usize % inserts.len()];
            v.replace_utf16(from16, to16, insert).unwrap();
            baseline.replace_range(start..end, insert);
            assert_eq!(v.text(), baseline);
            check(&v);
        }
    }
    #[test]
    fn twenty_mibibytes_stay_in_bounded_model_storage() {
        let block = "Lorem ipsum русский текст 🌍\n";
        let target = 20 * 1024 * 1024;
        let v = LineViewport::new(block.repeat(target / block.len() + 1));
        let visible = v.visible_text(v.line_count().saturating_sub(75), 60);
        assert!(visible.len() < 4096);
        assert!(v.bytes() > target);
        assert!(v.backing_capacity_bytes() < 2 * v.bytes());
        eprintln!(
            "AQUILUM_NATIVE_VIEWPORT text_bytes={} lines={} capacity_bytes={} visible_bytes={}",
            v.bytes(), v.line_count(), v.backing_capacity_bytes(), visible.len()
        );
    }
}
