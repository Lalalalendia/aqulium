use super::selection::Selection;
use crate::documents::merge::TextEdit;

pub const COALESCE_WINDOW_MS: u64 = 400;
const MAX_HISTORY_ENTRIES: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    InsertChar,
    InsertNewline,
    Backspace,
    Delete,
    Paste,
    Cut,
    ImeCommit,
    Indent,
    Replace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoEntry {
    pub kind: EditKind,
    pub timestamp_ms: u64,
    pub before_selection: Selection,
    pub after_selection: Selection,
    pub char_from: usize,
    pub utf16_from: usize,
    pub inserted_text: String,
    pub removed_text: String,
}

impl UndoEntry {
    /// Returns the UTF-16 `TextEdit` that undoes this entry.
    pub fn inverse_utf16_edit(&self) -> TextEdit {
        let inserted_utf16: usize = self.inserted_text.chars().map(char::len_utf16).sum();
        TextEdit {
            from: self.utf16_from,
            to: self.utf16_from + inserted_utf16,
            insert: self.removed_text.clone(),
        }
    }

    /// Returns the UTF-16 `TextEdit` that redoes this entry.
    pub fn forward_utf16_edit(&self) -> TextEdit {
        let removed_utf16: usize = self.removed_text.chars().map(char::len_utf16).sum();
        TextEdit {
            from: self.utf16_from,
            to: self.utf16_from + removed_utf16,
            insert: self.inserted_text.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    undo: Vec<UndoEntry>,
    redo: Vec<UndoEntry>,
    can_coalesce: bool,
}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn break_coalescing(&mut self) {
        self.can_coalesce = false;
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.can_coalesce = false;
    }

    pub fn push(&mut self, entry: UndoEntry) {
        self.redo.clear();
        if self.can_coalesce {
            if let Some(last) = self.undo.last_mut() {
                if should_coalesce(last, &entry) {
                    last.inserted_text.push_str(&entry.inserted_text);
                    last.after_selection = entry.after_selection;
                    last.timestamp_ms = entry.timestamp_ms;
                    return;
                }
            }
        }
        self.can_coalesce = entry.kind == EditKind::InsertChar;
        self.undo.push(entry);
        if self.undo.len() > MAX_HISTORY_ENTRIES {
            self.undo.remove(0);
        }
    }

    pub fn pop_undo(&mut self) -> Option<UndoEntry> {
        self.can_coalesce = false;
        let entry = self.undo.pop()?;
        self.redo.push(entry.clone());
        Some(entry)
    }

    pub fn pop_redo(&mut self) -> Option<UndoEntry> {
        self.can_coalesce = false;
        let entry = self.redo.pop()?;
        self.undo.push(entry.clone());
        Some(entry)
    }
}

fn should_coalesce(prev: &UndoEntry, next: &UndoEntry) -> bool {
    if prev.kind != EditKind::InsertChar || next.kind != EditKind::InsertChar {
        return false;
    }
    if !prev.removed_text.is_empty() || !next.removed_text.is_empty() {
        return false;
    }
    if next.timestamp_ms.saturating_sub(prev.timestamp_ms) > COALESCE_WINDOW_MS {
        return false;
    }
    let prev_end_char = prev.char_from + prev.inserted_text.chars().count();
    if next.char_from != prev_end_char {
        return false;
    }
    if next.inserted_text.contains('\n') {
        return false;
    }
    // Start a fresh undo group when transitioning from word characters to whitespace
    let prev_ends_space = prev
        .inserted_text
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace);
    let next_starts_space = next
        .inserted_text
        .chars()
        .next()
        .is_some_and(char::is_whitespace);
    if prev_ends_space && !next_starts_space {
        return false;
    }
    true
}
