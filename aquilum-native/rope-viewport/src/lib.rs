//! Memory-bounded Markdown editor data model, NOT yet a replacement GPUI UI.
//!
//! The Rope stores text once; widgets only receive a bounded UTF-8 window.
//! Windows carry a revision and character offset. An edit made against a stale
//! window is rejected rather than editing at the wrong position.
//! UTF-16 offsets follow CodeMirror/Yrs semantics; surrogate boundaries are
//! rejected by aquilum-native-common::text_positions.
//!
//! Production integration still requires real GUI IME, native undo history,
//! permission-aware delta commits to DocumentHub and crash recovery.

use aquilum_native_common::text_positions::{range_utf16_to_bytes, PositionError};
use ropey::Rope;
use std::io::{self, Read, Write};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    StaleViewport,
    InvalidPosition(PositionError),
    InvalidLine,
    InvalidWindowBudget,
}

#[derive(Debug, Clone)]
pub struct Viewport {
    pub revision: u64,
    pub start_char: usize,
    pub end_char: usize,
    pub start_line: usize,
    /// At most max_chars Unicode scalar values, not a whole document.
    pub text: String,
}

pub struct TextModel {
    rope: Rope,
    revision: u64,
}

impl TextModel {
    pub fn from_str(text: &str) -> Self {
        Self { rope: Rope::from_str(text), revision: 0 }
    }

    /// Stream UTF-8 input directly into rope chunks; no full-string copy.
    pub fn from_reader(reader: impl Read) -> io::Result<Self> {
        Ok(Self { rope: Rope::from_reader(reader)?, revision: 0 })
    }

    pub fn revision(&self) -> u64 { self.revision }
    pub fn len_bytes(&self) -> usize { self.rope.len_bytes() }
    pub fn len_chars(&self) -> usize { self.rope.len_chars() }
    pub fn len_lines(&self) -> usize { self.rope.len_lines() }

    /// Exactly one small, editable window. Safe for huge/one-line files.
    /// The window can end mid-line; the caller scrolls using char offsets.
    pub fn window_at_char(
        &self,
        start_char: usize,
        max_chars: usize,
    ) -> Result<Viewport, ModelError> {
        if max_chars == 0 { return Err(ModelError::InvalidWindowBudget); }
        if start_char > self.rope.len_chars() { return Err(ModelError::InvalidLine); }
        let end_char = start_char.saturating_add(max_chars).min(self.rope.len_chars());
        Ok(Viewport {
            revision: self.revision,
            start_char,
            end_char,
            start_line: self.rope.char_to_line(start_char),
            text: self.rope.slice(start_char..end_char).to_string(),
        })
    }

    /// Bounded by BOTH number of lines and number of Unicode characters.
    pub fn window_at_line(
        &self,
        start_line: usize,
        max_lines: usize,
        max_chars: usize,
    ) -> Result<Viewport, ModelError> {
        if max_lines == 0 || max_chars == 0 {
            return Err(ModelError::InvalidWindowBudget);
        }
        if start_line >= self.rope.len_lines() {
            return Err(ModelError::InvalidLine);
        }
        let start_char = self.rope.line_to_char(start_line);
        let next_line = start_line.saturating_add(max_lines).min(self.rope.len_lines());
        let end_limit = self.rope.line_to_char(next_line);
        let end_char = start_char.saturating_add(max_chars).min(end_limit);
        Ok(Viewport {
            revision: self.revision,
            start_char,
            end_char,
            start_line,
            text: self.rope.slice(start_char..end_char).to_string(),
        })
    }

    /// Apply a document change originating in the bounded editor viewport.
    /// Offsets are UTF-16 code units within viewport.text, NOT document-global.
    /// Do not call with arbitrary text from another revision.
    pub fn replace_in_viewport(
        &mut self,
        viewport: &Viewport,
        utf16_range: Range<usize>,
        replacement: &str,
    ) -> Result<u64, ModelError> {
        if viewport.revision != self.revision {
            return Err(ModelError::StaleViewport);
        }
        let range = range_utf16_to_bytes(&viewport.text, utf16_range.start, utf16_range.end)
            .map_err(ModelError::InvalidPosition)?;
        let first_local_char = viewport.text[..range.start].chars().count();
        let last_local_char = viewport.text[..range.end].chars().count();
        let first = viewport.start_char + first_local_char;
        let last = viewport.start_char + last_local_char;
        // An immutable viewport whose revision matches must be a true slice.
        // A malicious/inconsistent viewport must not edit unrelated content.
        if viewport.end_char != viewport.start_char + viewport.text.chars().count()
            || viewport.end_char > self.rope.len_chars()
            || self.rope.slice(viewport.start_char..viewport.end_char) != viewport.text.as_str()
        {
            return Err(ModelError::StaleViewport);
        }

        self.rope.remove(first..last);
        self.rope.insert(first, replacement);
        self.revision = self.revision.checked_add(1).expect("revision overflow");
        Ok(self.revision)
    }

    /// Stream to a writer without copying the entire Rope into a String.
    /// DO NOT write directly to real user files; the current production
    /// DocumentHub remains the sole version/durability authority.
    pub fn write_to(&self, mut out: impl Write) -> io::Result<()> {
        for block in self.rope.chunks() {
            out.write_all(block.as_bytes())?;
        }
        Ok(())
    }

    pub fn into_string(self) -> String { self.rope.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump(m: &TextModel) -> String {
        let mut bytes=Vec::new();
        m.write_to(&mut bytes).unwrap();
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn unicode_and_crlf_roundtrip_without_full_document_cloning() {
        let source = "Привет 🇷🇺🌍\r\nстрока e\u{301} и 漢字\r\nфинал";
        let model=TextModel::from_reader(source.as_bytes()).unwrap();
        assert_eq!(dump(&model),source);
        assert_eq!(model.len_bytes(),source.len());
        assert!(model.len_lines()>=3);
        let a=model.window_at_line(1,2,128).unwrap();
        assert!(a.text.starts_with("строка"));
        assert!(a.text.contains("финал"));
    }

    #[test]
    fn refusing_invalid_utf16_surrogates_prevents_edit_corruption() {
        let mut model=TextModel::from_str("A🌍B\n");
        let window=model.window_at_line(0,3,100).unwrap();
        let bad=model.replace_in_viewport(&window,2..3,"!");
        assert_eq!(bad, Err(ModelError::InvalidPosition(PositionError::InsideUtf16Surrogate)));
        assert_eq!(dump(&model),"A🌍B\n");
        let next=model.replace_in_viewport(&window,1..3,"👍🏼");
        assert_eq!(next,Ok(1));
        assert_eq!(dump(&model),"A👍🏼B\n");
    }

    #[test]
    fn rejects_stale_viewport_even_when_edit_coordinates_fit() {
        let mut model=TextModel::from_str("один\nдва\nтри");
        let first=model.window_at_line(0,2,16).unwrap();
        assert_eq!(model.replace_in_viewport(&first,0..0,"X"),Ok(1));
        assert_eq!(model.replace_in_viewport(&first,0..0,"Y"),Err(ModelError::StaleViewport));
        assert_eq!(dump(&model),"Xодин\nдва\nтри");
    }

    #[test]
    fn viewport_never_materializes_a_huge_line() {
        let source="x".repeat(5_000_000);
        let model=TextModel::from_str(&source);
        let view=model.window_at_line(0,200,8192).unwrap();
        assert_eq!(view.text.len(),8192);
        let late=model.window_at_char(4_990_000,8192).unwrap();
        assert_eq!(late.text.len(),8192);
        assert_eq!(late.start_char,4_990_000);
        assert_eq!(model.len_bytes(),5_000_000);
    }

    #[test]
    fn editing_late_page_never_rewrites_earlier_text() {
        let text="a\n".repeat(50_000)+"Конец 🌍\n";
        let mut model=TextModel::from_str(&text);
        let final_line=model.len_lines()-2;
        let view=model.window_at_line(final_line,2,100).unwrap();
        assert_eq!(view.text,"Конец 🌍\n");
        assert_eq!(model.replace_in_viewport(&view,6..8,"🌎"),Ok(1));
        let out=dump(&model);
        assert_eq!(out,"a\n".repeat(50_000)+"Конец 🌎\n");
    }

    #[test]
    fn empty_file_and_exact_line_window() {
        let mut model=TextModel::from_str("");
        let v=model.window_at_line(0,256,2048).unwrap();
        assert_eq!(v.text,"");
        model.replace_in_viewport(&v,0..0,"hello\n").unwrap();
        assert_eq!(model.len_lines(),2);
        assert_eq!(dump(&model),"hello\n");
        let last=model.window_at_line(1,1,50).unwrap();
        assert_eq!(last.text,"");
        assert_eq!(model.window_at_line(2,1,50).err(),Some(ModelError::InvalidLine));
    }

    #[test]
    fn invalid_bounds_or_mutated_viewport_are_not_accepted() {
        let mut model=TextModel::from_str("A 🌍 end");
        assert_eq!(model.window_at_char(0,0).err(),Some(ModelError::InvalidWindowBudget));
        let v=model.window_at_line(0,1,3).unwrap();
        let mut fabricated=v.clone();
        fabricated.text.push('!');
        assert_eq!(model.replace_in_viewport(&fabricated,0..0,"z"),Err(ModelError::StaleViewport));
        assert_eq!(dump(&model),"A 🌍 end");
    }
}
