use super::history::{EditKind, UndoEntry, UndoStack};
use super::ime::ImeState;
use super::rope::{LineCol, RopeBuffer};
use super::selection::{next_word_boundary, prev_word_boundary, Selection};
use super::viewport::{compute_viewport, ViewportWindow, MAX_VIEWPORT_CHARS};
use crate::app_core::Core;
use crate::documents::changes::edits_from_json;
use crate::documents::hub::{HubError, PullResult};
use crate::documents::merge::TextEdit;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub enum EditorError {
    Hub(HubError),
    ChangeSync(String),
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hub(err) => write!(f, "document hub error: {err:?}"),
            Self::ChangeSync(msg) => write!(f, "change sync error: {msg}"),
        }
    }
}

impl From<HubError> for EditorError {
    fn from(err: HubError) -> Self {
        Self::Hub(err)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyCommand {
    InsertText(String),
    InsertNewline,
    InsertTab,
    OutdentTab,
    Backspace,
    Delete,
    DeleteWordBackward,
    DeleteWordForward,
    MoveLeft { select: bool },
    MoveRight { select: bool },
    MoveUp { select: bool },
    MoveDown { select: bool },
    MoveWordLeft { select: bool },
    MoveWordRight { select: bool },
    MoveLineStart { select: bool },
    MoveLineEnd { select: bool },
    MoveDocStart { select: bool },
    MoveDocEnd { select: bool },
    PageUp { select: bool, lines: usize },
    PageDown { select: bool, lines: usize },
    SelectAll,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste(String),
    Save,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorMetrics {
    pub rope_allocated_bytes: usize,
    pub rope_len_bytes: usize,
    pub rope_len_chars: usize,
    pub rope_len_utf16: usize,
    pub rope_len_lines: usize,
    pub rope_chunk_count: usize,
    pub viewport_len_bytes: usize,
    pub viewport_len_chars: usize,
    pub version: u64,
    pub dirty: bool,
}

/// Stateful native Rust editor session backed by a chunked `RopeBuffer`,
/// bounded `ViewportWindow` (`<= 8192` chars), transactional `UndoStack`,
/// `ImeState`, and direct `DocumentHub` Yrs CRDT persistence.
pub struct NativeEditorSession {
    core: Arc<Core>,
    path: PathBuf,
    client_id: String,
    version: u64,
    rope: RopeBuffer,
    selection: Selection,
    ime: ImeState,
    history: UndoStack,
    clipboard: String,
    scroll_anchor_char: Option<usize>,
    viewport_budget_chars: usize,
    clock_ms: u64,
}

impl NativeEditorSession {
    pub fn open(
        core: Arc<Core>,
        path: impl AsRef<Path>,
        client_id: impl Into<String>,
    ) -> Result<Self, EditorError> {
        let path = path.as_ref().to_path_buf();
        let opened = core.documents.open(&core, &path, 0)?;
        let rope = RopeBuffer::from_text(&opened.text);
        core.active_note.set(Some(path.clone()));
        Ok(Self {
            core,
            path,
            client_id: client_id.into(),
            version: opened.version,
            rope,
            selection: Selection::caret(0),
            ime: ImeState::default(),
            history: UndoStack::new(),
            clipboard: String::new(),
            scroll_anchor_char: None,
            viewport_budget_chars: MAX_VIEWPORT_CHARS,
            clock_ms: 0,
        })
    }

    pub fn core(&self) -> &Arc<Core> {
        &self.core
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn replace_char_range_atomic(
        &mut self,
        from_char: usize,
        to_char: usize,
        insert: &str,
        kind: EditKind,
    ) -> Result<(), EditorError> {
        self.finish_ime_if_active();
        self.apply_user_edit(from_char, to_char, insert, kind)
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn rope(&self) -> &RopeBuffer {
        &self.rope
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn ime(&self) -> &ImeState {
        &self.ime
    }

    pub fn history(&self) -> &UndoStack {
        &self.history
    }

    pub fn clipboard(&self) -> &str {
        &self.clipboard
    }

    pub fn set_viewport_budget_chars(&mut self, max_chars: usize) {
        self.viewport_budget_chars = max_chars.clamp(64, MAX_VIEWPORT_CHARS);
    }

    pub fn advance_clock_ms(&mut self, delta_ms: u64) {
        self.clock_ms = self.clock_ms.saturating_add(delta_ms);
    }

    pub fn set_selection(&mut self, selection: Selection) {
        self.finish_ime_if_active();
        self.selection = selection.clamp(self.rope.len_chars());
        self.scroll_anchor_char = None;
        self.history.break_coalescing();
    }

    /// Selects text using viewport-relative character offsets (e.g. from GPUI mouse drag).
    pub fn select_in_viewport(
        &mut self,
        viewport: &ViewportWindow,
        anchor_in_viewport_chars: usize,
        head_in_viewport_chars: usize,
    ) {
        let doc_anchor = (viewport.start_char + anchor_in_viewport_chars).min(self.rope.len_chars());
        let doc_head = (viewport.start_char + head_in_viewport_chars).min(self.rope.len_chars());
        self.set_selection(Selection::new(doc_anchor, doc_head));
    }

    pub fn scroll_to_line(&mut self, line: usize) {
        let (line_start, _, _) = self.rope.line_char_bounds(line);
        self.scroll_anchor_char = Some(line_start);
    }

    pub fn clear_scroll_anchor(&mut self) {
        self.scroll_anchor_char = None;
    }

    /// Computes the bounded GPUI viewport (`<= 8192` Unicode scalars) around the active
    /// cursor or scroll position.
    pub fn viewport(&self) -> ViewportWindow {
        compute_viewport(
            &self.rope,
            &self.selection,
            &self.ime,
            self.scroll_anchor_char,
            self.viewport_budget_chars,
        )
    }

    pub fn metrics(&self) -> EditorMetrics {
        let vp = self.viewport();
        let dirty = self.core.documents.is_dirty(&self.path).unwrap_or(false);
        EditorMetrics {
            rope_allocated_bytes: self.rope.allocated_bytes(),
            rope_len_bytes: self.rope.len_bytes(),
            rope_len_chars: self.rope.len_chars(),
            rope_len_utf16: self.rope.len_utf16(),
            rope_len_lines: self.rope.len_lines(),
            rope_chunk_count: self.rope.chunk_count(),
            viewport_len_bytes: vp.text.len(),
            viewport_len_chars: vp.text.chars().count(),
            version: self.version,
            dirty,
        }
    }

    /// Updates live Input Method Editor (IME) composition (`preedit`) inside the Rope
    /// and Viewport without polluting the Undo stack or sending uncommitted strokes to disk.
    pub fn ime_preedit(
        &mut self,
        text: &str,
        cursor_in_preedit: Option<(usize, usize)>,
    ) {
        self.history.break_coalescing();
        self.scroll_anchor_char = None;

        let preedit_chars = text.chars().count();
        let start_char = if let Some((active_start, active_end)) = self.ime.active_range {
            self.rope.replace_char_range(active_start, active_end, text);
            active_start
        } else {
            self.ime.initial_selection = self.selection;
            let (sel_start, sel_end) = self.selection.range();
            self.ime.replaced_text = self.rope.slice_chars(sel_start, sel_end);
            self.rope.replace_char_range(sel_start, sel_end, text);
            sel_start
        };

        let end_char = start_char + preedit_chars;
        self.ime.active_range = Some((start_char, end_char));
        self.ime.preedit_text = text.to_owned();
        self.ime.cursor_in_preedit = cursor_in_preedit;

        let caret_offset = cursor_in_preedit
            .map_or(preedit_chars, |(_, c_end)| c_end.min(preedit_chars));
        self.selection = Selection::caret(start_char + caret_offset);
    }

    /// Cancels an active IME composition and restores the original selection and text.
    pub fn ime_cancel(&mut self) {
        if let Some((active_start, active_end)) = self.ime.active_range {
            let replaced = self.ime.replaced_text.clone();
            let initial_sel = self.ime.initial_selection;
            self.rope.replace_char_range(active_start, active_end, &replaced);
            self.selection = initial_sel.clamp(self.rope.len_chars());
            self.ime.clear();
        }
    }

    /// Commits the final IME composition string as a single atomic edit in both
    /// `UndoStack` and `DocumentHub`.
    pub fn ime_commit(&mut self, committed_text: &str) -> Result<(), EditorError> {
        if self.ime.is_composing() {
            self.ime_cancel();
        }
        if committed_text.is_empty() && self.selection.is_empty() {
            return Ok(());
        }
        let (from_char, to_char) = self.selection.range();
        self.apply_user_edit(from_char, to_char, committed_text, EditKind::ImeCommit)?;
        self.history.break_coalescing();
        Ok(())
    }

    fn finish_ime_if_active(&mut self) {
        if self.ime.is_composing() {
            let text = self.ime.preedit_text.clone();
            let _ = self.ime_commit(&text);
        }
    }

    pub fn handle_key(&mut self, command: KeyCommand) -> Result<(), EditorError> {
        self.finish_ime_if_active();
        self.scroll_anchor_char = None;

        match command {
            KeyCommand::InsertText(text) => {
                if text.is_empty() {
                    return Ok(());
                }
                let (from_char, to_char) = self.selection.range();
                let kind = if text.chars().count() == 1 && !text.contains('\n') && from_char == to_char {
                    EditKind::InsertChar
                } else {
                    EditKind::Replace
                };
                self.apply_user_edit(from_char, to_char, &text, kind)
            }
            KeyCommand::InsertNewline => {
                let (from_char, to_char) = self.selection.range();
                let continuation = self.markdown_line_continuation(from_char);
                if let Some((replace_from, replace_to, replacement)) = continuation {
                    self.apply_user_edit(replace_from, replace_to, &replacement, EditKind::InsertNewline)
                } else {
                    self.apply_user_edit(from_char, to_char, "\n", EditKind::InsertNewline)
                }
            }
            KeyCommand::InsertTab => {
                let (from_char, to_char) = self.selection.range();
                self.apply_user_edit(from_char, to_char, "    ", EditKind::Indent)
            }
            KeyCommand::OutdentTab => {
                let lc = self.rope.char_to_line_col(self.selection.start());
                let (line_start, line_end, _) = self.rope.line_char_bounds(lc.line);
                let line_text = self.rope.slice_chars(line_start, (line_start + 4).min(line_end));
                let spaces = line_text.chars().take_while(|&c| c == ' ').count();
                if spaces > 0 {
                    self.apply_user_edit(line_start, line_start + spaces, "", EditKind::Indent)?;
                } else if line_text.starts_with('\t') {
                    self.apply_user_edit(line_start, line_start + 1, "", EditKind::Indent)?;
                }
                Ok(())
            }
            KeyCommand::Backspace => {
                let (from_char, to_char) = self.selection.range();
                if from_char < to_char {
                    self.apply_user_edit(from_char, to_char, "", EditKind::Backspace)
                } else if from_char > 0 {
                    let prev_char = step_prev_grapheme_cluster(&self.rope, from_char);
                    self.apply_user_edit(prev_char, from_char, "", EditKind::Backspace)
                } else {
                    Ok(())
                }
            }
            KeyCommand::Delete => {
                let (from_char, to_char) = self.selection.range();
                if from_char < to_char {
                    self.apply_user_edit(from_char, to_char, "", EditKind::Delete)
                } else if to_char < self.rope.len_chars() {
                    let next_char = step_next_grapheme_cluster(&self.rope, to_char);
                    self.apply_user_edit(from_char, next_char, "", EditKind::Delete)
                } else {
                    Ok(())
                }
            }
            KeyCommand::DeleteWordBackward => {
                let (from_char, to_char) = self.selection.range();
                if from_char < to_char {
                    self.apply_user_edit(from_char, to_char, "", EditKind::Backspace)
                } else if from_char > 0 {
                    let word_start = prev_word_boundary(&self.rope, from_char);
                    self.apply_user_edit(word_start, from_char, "", EditKind::Backspace)
                } else {
                    Ok(())
                }
            }
            KeyCommand::DeleteWordForward => {
                let (from_char, to_char) = self.selection.range();
                if from_char < to_char {
                    self.apply_user_edit(from_char, to_char, "", EditKind::Delete)
                } else if to_char < self.rope.len_chars() {
                    let word_end = next_word_boundary(&self.rope, to_char);
                    self.apply_user_edit(from_char, word_end, "", EditKind::Delete)
                } else {
                    Ok(())
                }
            }
            KeyCommand::MoveLeft { select } => {
                self.history.break_coalescing();
                if !select && !self.selection.is_empty() {
                    self.selection = Selection::caret(self.selection.start());
                } else {
                    let target = step_prev_grapheme_cluster(&self.rope, self.selection.head);
                    self.move_head_to(target, select, None);
                }
                Ok(())
            }
            KeyCommand::MoveRight { select } => {
                self.history.break_coalescing();
                if !select && !self.selection.is_empty() {
                    self.selection = Selection::caret(self.selection.end());
                } else {
                    let target = step_next_grapheme_cluster(&self.rope, self.selection.head);
                    self.move_head_to(target, select, None);
                }
                Ok(())
            }
            KeyCommand::MoveWordLeft { select } => {
                self.history.break_coalescing();
                let target = prev_word_boundary(&self.rope, self.selection.head);
                self.move_head_to(target, select, None);
                Ok(())
            }
            KeyCommand::MoveWordRight { select } => {
                self.history.break_coalescing();
                let target = next_word_boundary(&self.rope, self.selection.head);
                self.move_head_to(target, select, None);
                Ok(())
            }
            KeyCommand::MoveUp { select } => {
                self.history.break_coalescing();
                self.move_vertical(-1, select);
                Ok(())
            }
            KeyCommand::MoveDown { select } => {
                self.history.break_coalescing();
                self.move_vertical(1, select);
                Ok(())
            }
            KeyCommand::PageUp { select, lines } => {
                self.history.break_coalescing();
                self.move_vertical(-(lines.max(1) as isize), select);
                Ok(())
            }
            KeyCommand::PageDown { select, lines } => {
                self.history.break_coalescing();
                self.move_vertical(lines.max(1) as isize, select);
                Ok(())
            }
            KeyCommand::MoveLineStart { select } => {
                self.history.break_coalescing();
                let lc = self.rope.char_to_line_col(self.selection.head);
                let (line_start, line_end, _) = self.rope.line_char_bounds(lc.line);
                let line_slice = self.rope.slice_chars(line_start, line_end);
                let first_non_ws = line_slice.chars().take_while(|c| c.is_whitespace()).count();
                let smart_home = line_start + first_non_ws;
                let target = if self.selection.head == smart_home && first_non_ws > 0 {
                    line_start
                } else {
                    smart_home
                };
                self.move_head_to(target, select, Some(target - line_start));
                Ok(())
            }
            KeyCommand::MoveLineEnd { select } => {
                self.history.break_coalescing();
                let lc = self.rope.char_to_line_col(self.selection.head);
                let (line_start, line_end, _) = self.rope.line_char_bounds(lc.line);
                self.move_head_to(line_end, select, Some(line_end - line_start));
                Ok(())
            }
            KeyCommand::MoveDocStart { select } => {
                self.history.break_coalescing();
                self.move_head_to(0, select, Some(0));
                Ok(())
            }
            KeyCommand::MoveDocEnd { select } => {
                self.history.break_coalescing();
                let end = self.rope.len_chars();
                self.move_head_to(end, select, None);
                Ok(())
            }
            KeyCommand::SelectAll => {
                self.history.break_coalescing();
                self.selection = Selection::new(0, self.rope.len_chars());
                Ok(())
            }
            KeyCommand::Copy => {
                let (from_char, to_char) = self.selection.range();
                if from_char < to_char {
                    self.clipboard = self.rope.slice_chars(from_char, to_char);
                }
                Ok(())
            }
            KeyCommand::Cut => {
                let (from_char, to_char) = self.selection.range();
                if from_char < to_char {
                    self.clipboard = self.rope.slice_chars(from_char, to_char);
                    self.apply_user_edit(from_char, to_char, "", EditKind::Cut)?;
                }
                Ok(())
            }
            KeyCommand::Paste(text) => {
                let paste_text = if text.is_empty() {
                    self.clipboard.clone()
                } else {
                    text
                };
                if paste_text.is_empty() {
                    return Ok(());
                }
                let (from_char, to_char) = self.selection.range();
                self.apply_user_edit(from_char, to_char, &paste_text, EditKind::Paste)?;
                self.history.break_coalescing();
                Ok(())
            }
            KeyCommand::Undo => self.undo(),
            KeyCommand::Redo => self.redo(),
            KeyCommand::Save => {
                self.save_now()?;
                Ok(())
            }
        }
    }

    fn move_head_to(&mut self, target_char: usize, select: bool, preferred_column: Option<usize>) {
        let clamped = target_char.min(self.rope.len_chars());
        let anchor = if select { self.selection.anchor } else { clamped };
        self.selection = Selection {
            anchor,
            head: clamped,
            preferred_column,
        };
    }

    fn move_vertical(&mut self, delta_lines: isize, select: bool) {
        let lc = self.rope.char_to_line_col(self.selection.head);
        let preferred_col = self.selection.preferred_column.unwrap_or(lc.column_chars);
        let max_line = self.rope.len_lines().saturating_sub(1);
        let target_line = if delta_lines < 0 {
            lc.line.saturating_sub((-delta_lines) as usize)
        } else {
            (lc.line + (delta_lines as usize)).min(max_line)
        };
        let target_char = self.rope.line_col_to_char(LineCol {
            line: target_line,
            column_chars: preferred_col,
        });
        self.move_head_to(target_char, select, Some(preferred_col));
    }

    /// Smart Markdown list / task / blockquote continuation on Enter.
    fn markdown_line_continuation(&self, caret_char: usize) -> Option<(usize, usize, String)> {
        if !self.selection.is_empty() {
            return None;
        }
        let lc = self.rope.char_to_line_col(caret_char);
        let (line_start, line_end, _) = self.rope.line_char_bounds(lc.line);
        if caret_char < line_end {
            return None;
        }
        let line = self.rope.slice_chars(line_start, line_end);
        let indent_chars = line.chars().take_while(|&c| c == ' ' || c == '\t').count();
        let indent: String = line.chars().take(indent_chars).collect();
        let rest: String = line.chars().skip(indent_chars).collect();

        for prefix in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] ", "- ", "* ", "> "] {
            if let Some(after) = rest.strip_prefix(prefix) {
                if after.trim().is_empty() {
                    // Pressing Enter on an empty list/quote marker exits the list/quote
                    return Some((line_start, line_end, String::new()));
                }
                let next_marker = match prefix {
                    "- [x] " | "- [X] " => "- [ ] ",
                    "* [x] " => "* [ ] ",
                    other => other,
                };
                return Some((caret_char, caret_char, format!("\n{indent}{next_marker}")));
            }
        }
        None
    }

    fn apply_user_edit(
        &mut self,
        from_char: usize,
        to_char: usize,
        insert: &str,
        kind: EditKind,
    ) -> Result<(), EditorError> {
        let before_selection = self.selection;
        let (utf16_edit, removed_text) = self.rope.replace_char_range(from_char, to_char, insert);
        let inserted_chars = insert.chars().count();
        let after_selection = Selection::caret(from_char + inserted_chars);
        self.selection = after_selection;

        self.push_edit_to_hub(&utf16_edit)?;

        self.history.push(UndoEntry {
            kind,
            timestamp_ms: self.clock_ms,
            before_selection,
            after_selection,
            char_from: from_char,
            utf16_from: utf16_edit.from,
            inserted_text: insert.to_owned(),
            removed_text,
        });
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), EditorError> {
        self.finish_ime_if_active();
        let Some(entry) = self.history.pop_undo() else {
            return Ok(());
        };
        let inverse_edit = entry.inverse_utf16_edit();
        self.rope.apply_utf16_edit(&inverse_edit);
        self.selection = entry.before_selection.clamp(self.rope.len_chars());
        self.push_edit_to_hub(&inverse_edit)?;
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), EditorError> {
        self.finish_ime_if_active();
        let Some(entry) = self.history.pop_redo() else {
            return Ok(());
        };
        let forward_edit = entry.forward_utf16_edit();
        self.rope.apply_utf16_edit(&forward_edit);
        self.selection = entry.after_selection.clamp(self.rope.len_chars());
        self.push_edit_to_hub(&forward_edit)?;
        Ok(())
    }

    fn push_edit_to_hub(&mut self, edit: &TextEdit) -> Result<(), EditorError> {
        if edit.from == edit.to && edit.insert.is_empty() {
            return Ok(());
        }
        let edits = [edit.clone()];
        let accepted = self.core.documents.push_edits(
            &self.core,
            &self.path,
            self.version,
            &self.client_id,
            &edits,
        )?;
        if accepted {
            self.version += 1;
            return Ok(());
        }
        // Version advanced externally (e.g. disk watcher or MCP); pull and resync
        self.sync_external()?;
        Ok(())
    }

    /// Pulls any external changes from `DocumentHub` (e.g., disk watcher or MCP edits)
    /// and applies them incrementally to the `RopeBuffer` while preserving selection.
    pub fn sync_external(&mut self) -> Result<(), EditorError> {
        self.finish_ime_if_active();
        match self.core.documents.pull(&self.path, self.version)? {
            PullResult::Changes { changes } => {
                for logged in changes {
                    let doc_utf16_len = self.rope.len_utf16();
                    let edits = edits_from_json(&logged.changes, doc_utf16_len)
                        .map_err(|e| EditorError::ChangeSync(e.to_string()))?;
                    for edit in edits.iter().rev() {
                        let (from_char, new_to_char, removed) = self.rope.apply_utf16_edit(edit);
                        let old_to_char = from_char + removed.chars().count();
                        let inserted_chars = new_to_char - from_char;
                        self.selection = self.selection.map_through_edit(
                            from_char,
                            old_to_char,
                            inserted_chars,
                            self.rope.len_chars(),
                        );
                    }
                    self.version += 1;
                }
                self.history.break_coalescing();
            }
            PullResult::Resync { text, version } => {
                self.rope = RopeBuffer::from_text(&text);
                self.selection = self.selection.clamp(self.rope.len_chars());
                self.version = version;
                self.history.clear();
            }
        }
        Ok(())
    }

    /// Immediately flushes all pending edits to disk and compacts the SQLite CRDT replica.
    pub fn save_now(&mut self) -> Result<u64, EditorError> {
        self.finish_ime_if_active();
        self.history.break_coalescing();
        let saved_version = self.core.documents.flush(&self.core, &self.path)?;
        self.version = saved_version;
        Ok(saved_version)
    }

    /// Releases the session in `DocumentHub` (which flushes any remaining unsaved edits)
    /// and reopens it from disk + SQLite store, verifying zero data loss.
    pub fn reopen(&mut self) -> Result<(), EditorError> {
        self.finish_ime_if_active();
        self.core.documents.release(&self.core, &self.path);
        let opened = self.core.documents.open(&self.core, &self.path, 0)?;
        self.rope = RopeBuffer::from_text(&opened.text);
        self.version = opened.version;
        self.selection = self.selection.clamp(self.rope.len_chars());
        self.history.clear();
        Ok(())
    }

    pub fn close(mut self) {
        self.finish_ime_if_active();
        self.core.documents.release(&self.core, &self.path);
    }
}

fn is_combining_or_variation(ch: char) -> bool {
    let u = ch as u32;
    (0x0300..=0x036F).contains(&u)
        || (0x1AB0..=0x1AFF).contains(&u)
        || (0x1DC0..=0x1DFF).contains(&u)
        || (0x20D0..=0x20FF).contains(&u)
        || (0xFE00..=0xFE0F).contains(&u)
        || (0xFE20..=0xFE2F).contains(&u)
        || u == 0x200D
}

fn step_prev_grapheme_cluster(rope: &RopeBuffer, from_char: usize) -> usize {
    let mut idx = from_char.min(rope.len_chars());
    if idx == 0 {
        return 0;
    }
    idx -= 1;
    // Step across CRLF or trailing combining marks
    if rope.char_at(idx) == Some('\n') && idx > 0 && rope.char_at(idx - 1) == Some('\r') {
        return idx - 1;
    }
    while idx > 0 {
        let Some(ch) = rope.char_at(idx) else { break };
        if is_combining_or_variation(ch) {
            idx -= 1;
        } else {
            break;
        }
    }
    idx
}

fn step_next_grapheme_cluster(rope: &RopeBuffer, from_char: usize) -> usize {
    let len = rope.len_chars();
    let mut idx = from_char.min(len);
    if idx >= len {
        return len;
    }
    if rope.char_at(idx) == Some('\r') && idx + 1 < len && rope.char_at(idx + 1) == Some('\n') {
        return idx + 2;
    }
    idx += 1;
    while idx < len {
        let Some(ch) = rope.char_at(idx) else { break };
        if is_combining_or_variation(ch) {
            idx += 1;
        } else {
            break;
        }
    }
    idx
}
