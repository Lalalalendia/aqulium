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
    EmptyEdit,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for ModelError {}

#[derive(Debug, Clone)]
pub struct Viewport {
    pub revision: u64,
    pub start_char: usize,
    pub end_char: usize,
    pub start_line: usize,
    /// At most max_chars Unicode scalar values, not a whole document.
    pub text: String,
}

/// A prevalidated, not-yet-committed view edit. Send its compact Yrs/CodeMirror
/// JSON to DocumentHub FIRST, and only commit the rope on acceptance.
#[derive(Debug, Clone)]
pub struct ProposedEdit {
    revision: u64,
    start_char: usize,
    end_char: usize,
    old_utf16_length: usize,
    from_utf16: usize,
    to_utf16: usize,
    replacement: String,
}
impl ProposedEdit {
    /// CodeMirror ChangeSet JSON, identical to existing documentSync transport.
    /// Example: document "A🌍B", replace 🌍 with X -> [1,[2,"X"],1].
    /// Only the new text appears in this message, never the full document.
    pub fn to_changeset_json(&self) -> serde_json::Value {
        use serde_json::{json, Value};
        let mut sections = Vec::<Value>::with_capacity(3);
        if self.from_utf16 > 0 {
            sections.push(json!(self.from_utf16));
        }
        let mut replaced = vec![json!(self.to_utf16 - self.from_utf16)];
        if !self.replacement.is_empty() {
            replaced.extend(self.replacement.split('\n').map(|line| json!(line)));
        }
        sections.push(Value::Array(replaced));
        let tail = self.old_utf16_length - self.to_utf16;
        if tail > 0 {
            sections.push(json!(tail));
        }
        Value::Array(sections)
    }
    pub fn utf16_range(&self) -> Range<usize> { self.from_utf16..self.to_utf16 }
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
    /// Plan a small change against the current immutable view, never moving
    /// or copying the rest of the document. Stale/forged views are rejected.
    pub fn propose_viewport_change(
        &self,
        viewport: &Viewport,
        utf16_range: Range<usize>,
        replacement: &str,
    ) -> Result<ProposedEdit, ModelError> {
        if viewport.revision != self.revision {
            return Err(ModelError::StaleViewport);
        }
        let range = range_utf16_to_bytes(&viewport.text, utf16_range.start, utf16_range.end)
            .map_err(ModelError::InvalidPosition)?;
        if viewport.end_char != viewport.start_char + viewport.text.chars().count()
            || viewport.end_char > self.rope.len_chars()
            || self.rope.slice(viewport.start_char..viewport.end_char) != viewport.text.as_str()
        {
            return Err(ModelError::StaleViewport);
        }
        let start_char = viewport.start_char + viewport.text[..range.start].chars().count();
        let end_char = viewport.start_char + viewport.text[..range.end].chars().count();
        if start_char == end_char && replacement.is_empty() {
            return Err(ModelError::EmptyEdit);
        }
        Ok(ProposedEdit {
            revision: self.revision,
            start_char,
            end_char,
            old_utf16_length: self.rope.len_utf16_cu(),
            from_utf16: self.rope.char_to_utf16_cu(start_char),
            to_utf16: self.rope.char_to_utf16_cu(end_char),
            replacement: replacement.to_owned(),
        })
    }

    /// Turn the small edited viewport widget content into a single document
    /// edit. Comparisons use Unicode scalars, then express positions in UTF-16.
    /// Unmodified common prefix/suffix are preserved in CodeMirror's ChangeSet.
    ///
    /// A bounded widget may generate multiple disjoint changes before Save.
    /// They are represented as ONE contiguous replacement within the viewport,
    /// preserving the same final text. No full document copy is required.
    pub fn propose_edited_viewport_text(
        &self,
        viewport: &Viewport,
        edited: &str,
        max_edited_chars: usize,
    ) -> Result<Option<ProposedEdit>, ModelError> {
        if viewport.revision != self.revision {
            return Err(ModelError::StaleViewport);
        }
        if max_edited_chars == 0 || edited.chars().count() > max_edited_chars {
            return Err(ModelError::InvalidWindowBudget);
        }
        if edited == viewport.text {
            return Ok(None);
        }

        let before: Vec<char> = viewport.text.chars().collect();
        let after: Vec<char> = edited.chars().collect();
        let prefix = before.iter().zip(after.iter())
            .take_while(|(a, b)| a == b).count();

        let mut suffix = 0usize;
        while suffix < before.len() - prefix
            && suffix < after.len() - prefix
            && before[before.len() - 1 - suffix] == after[after.len() - 1 - suffix]
        {
            suffix += 1;
        }
        let from_utf16 = before[..prefix].iter().map(|ch| ch.len_utf16()).sum();
        let to_utf16 = before[..before.len() - suffix].iter().map(|ch| ch.len_utf16()).sum();
        let replacement: String = after[prefix..after.len() - suffix].iter().collect();
        self.propose_viewport_change(viewport, from_utf16..to_utf16, &replacement).map(Some)
    }

    /// Call ONLY after the real DocumentHub acknowledged this exact edit.
    /// In-memory model is not mutated on stale/failed external commits.
    pub fn commit_proposed_edit(&mut self, edit: ProposedEdit) -> Result<u64, ModelError> {
        if edit.revision != self.revision
            || self.rope.len_utf16_cu() != edit.old_utf16_length
        {
            return Err(ModelError::StaleViewport);
        }
        self.rope.remove(edit.start_char..edit.end_char);
        self.rope.insert(edit.start_char, &edit.replacement);
        self.revision = self.revision.checked_add(1).expect("revision overflow");
        Ok(self.revision)
    }

    /// Local-only helper for independent text-model tests; production code
    /// should propose, send to DocumentHub, then commit the same proposal.
    pub fn replace_in_viewport(
        &mut self,
        viewport: &Viewport,
        utf16_range: Range<usize>,
        replacement: &str,
    ) -> Result<u64, ModelError> {
        let planned = self.propose_viewport_change(viewport, utf16_range, replacement)?;
        self.commit_proposed_edit(planned)
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
    fn widget_text_diff_uses_utf16_and_preserves_surrounding_text() {
        let mut model = TextModel::from_str("A🌍B\nX");
        let view = model.window_at_char(0, 64).unwrap();
        let proposal = model.propose_edited_viewport_text(&view, "A🪐B\nX", 128)
            .unwrap().unwrap();
        assert_eq!(proposal.to_changeset_json(), serde_json::json!([1,[2,"🪐"],3]));
        model.commit_proposed_edit(proposal).unwrap();
        assert_eq!(dump(&model), "A🪐B\nX");
    }

    #[test]
    fn widget_disjoint_edits_fold_into_one_safe_contiguous_delta() {
        let mut model = TextModel::from_str("one two three");
        let view = model.window_at_char(0, 100).unwrap();
        let next = model.propose_edited_viewport_text(&view, "ONE two THREE", 128)
            .unwrap().unwrap();
        assert_eq!(next.utf16_range(), 0..13);
        model.commit_proposed_edit(next).unwrap();
        assert_eq!(dump(&model), "ONE two THREE");
    }

    #[test]
    fn widget_budget_and_stale_window_are_rejected_before_saving() {
        let mut model=TextModel::from_str("A🌍B");
        let before=model.window_at_char(0,64).unwrap();
        assert_eq!(model.propose_edited_viewport_text(&before, &"X".repeat(30),16).err(),
            Some(ModelError::InvalidWindowBudget));
        assert!(model.propose_edited_viewport_text(&before,"A🌍B",16).unwrap().is_none());
        model.replace_in_viewport(&before, 1..3, "X").unwrap();
        assert_eq!(model.propose_edited_viewport_text(&before,"A🌍B",16).err(),
            Some(ModelError::StaleViewport));
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
