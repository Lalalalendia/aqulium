//! In-Document Search & Replace (`Ctrl+F`), Inline Formatting Shortcuts (`Ctrl+B/I/Shift+S`),
//! Smart Typography (`--` -> `—`), Automatic URL Title Pasting, and `[[WikiLink]]` Autocomplete.

use super::editor::{EditorError, NativeEditorSession};
use super::history::EditKind;
use super::rope::RopeBuffer;
use super::selection::Selection;
use crate::link_title::fetch::page_title;
use crate::search::models::NoteSuggestion;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageSearchState {
    pub active: bool,
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub matches: Vec<(usize, usize)>,
    pub current_index: Option<usize>,
}

impl PageSearchState {
    pub fn recompute(&mut self, rope: &RopeBuffer, caret_char: usize) {
        self.matches.clear();
        if !self.active || self.query.is_empty() || rope.is_empty() {
            self.current_index = None;
            return;
        }
        let q_chars = self.query.chars().count();
        if q_chars == 0 {
            self.current_index = None;
            return;
        }

        // V2 Chunk-Streaming Search: scans across `rope.chunks()` with a tiny overlap buffer
        // for cross-chunk matches — 0 B full-document string or Vec<char> allocation!
        let needle_folded = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.chars().flat_map(char::to_lowercase).collect::<String>()
        };

        let mut chunk_start_char = 0usize;
        let mut prev_tail_text = String::new();
        let mut prev_tail_start_char = 0usize;

        for chunk in rope.chunks() {
            let chunk_chars = chunk.metrics.chars;
            if !prev_tail_text.is_empty() {
                let head_take = q_chars.saturating_sub(1).min(chunk_chars);
                let head_byte = chunk
                    .text
                    .char_indices()
                    .nth(head_take)
                    .map_or(chunk.text.len(), |(b, _)| b);
                let mut boundary = String::with_capacity(prev_tail_text.len() + head_byte);
                boundary.push_str(&prev_tail_text);
                boundary.push_str(&chunk.text[..head_byte]);
                let tail_chars = prev_tail_text.chars().count();
                collect_matches_in_slice(
                    &boundary,
                    &needle_folded,
                    self.case_sensitive,
                    q_chars,
                    prev_tail_start_char,
                    Some(tail_chars),
                    &mut self.matches,
                );
            }

            collect_matches_in_slice(
                &chunk.text,
                &needle_folded,
                self.case_sensitive,
                q_chars,
                chunk_start_char,
                None,
                &mut self.matches,
            );

            if q_chars > 1 {
                let keep_chars = (q_chars - 1).min(chunk_chars);
                let skip_chars = chunk_chars - keep_chars;
                let tail_byte = chunk
                    .text
                    .char_indices()
                    .nth(skip_chars)
                    .map_or(0, |(b, _)| b);
                prev_tail_text.clear();
                prev_tail_text.push_str(&chunk.text[tail_byte..]);
                prev_tail_start_char = chunk_start_char + skip_chars;
            }
            chunk_start_char += chunk_chars;
        }

        if self.matches.is_empty() {
            self.current_index = None;
        } else {
            let idx = self
                .matches
                .iter()
                .position(|&(start, _)| start >= caret_char)
                .unwrap_or(0);
            self.current_index = Some(idx);
        }
    }

    /// Returns match character ranges clipped and translated into `[vp_start_char..vp_end_char]`.
    pub fn visible_matches_in_viewport(
        &self,
        vp_start_char: usize,
        vp_end_char: usize,
    ) -> Vec<(usize, usize, bool)> {
        let mut out = Vec::new();
        for (idx, &(m_start, m_end)) in self.matches.iter().enumerate() {
            if m_end <= vp_start_char || m_start >= vp_end_char {
                continue;
            }
            let local_start = m_start.max(vp_start_char) - vp_start_char;
            let local_end = m_end.min(vp_end_char) - vp_start_char;
            let is_current = self.current_index == Some(idx);
            out.push((local_start, local_end, is_current));
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WikiAutocompletePopup {
    /// Character range of the `[[query` prefix in the `RopeBuffer` (starting right after `[[`).
    pub query_char_range: (usize, usize),
    pub has_closing_brackets: bool,
    pub query: String,
    pub suggestions: Vec<NoteSuggestion>,
}

impl NativeEditorSession {
    /// Searches the current document for `query` and selects the nearest match at or after the caret.
    pub fn find_in_page(&mut self, query: &str, case_sensitive: bool) -> PageSearchState {
        let mut state = PageSearchState {
            active: true,
            query: query.to_owned(),
            replacement: String::new(),
            case_sensitive,
            matches: Vec::new(),
            current_index: None,
        };
        state.recompute(self.rope(), self.selection().start());
        if let Some(idx) = state.current_index {
            let (s, e) = state.matches[idx];
            self.set_selection(Selection::new(s, e));
        }
        state
    }

    pub fn page_search_next(&mut self, state: &mut PageSearchState) -> Option<(usize, usize)> {
        state.recompute(self.rope(), self.selection().end());
        if state.matches.is_empty() {
            return None;
        }
        let idx = state.current_index.unwrap_or(0);
        let range = state.matches[idx];
        self.set_selection(Selection::new(range.0, range.1));
        Some(range)
    }

    pub fn page_search_prev(&mut self, state: &mut PageSearchState) -> Option<(usize, usize)> {
        state.recompute(self.rope(), self.selection().start());
        if state.matches.is_empty() {
            return None;
        }
        let caret = self.selection().start();
        let prev_idx = state
            .matches
            .iter()
            .rposition(|&(_, end)| end <= caret)
            .unwrap_or(state.matches.len() - 1);
        state.current_index = Some(prev_idx);
        let range = state.matches[prev_idx];
        self.set_selection(Selection::new(range.0, range.1));
        Some(range)
    }

    pub fn page_search_replace_current(
        &mut self,
        state: &mut PageSearchState,
        replacement: &str,
    ) -> Result<bool, EditorError> {
        state.recompute(self.rope(), self.selection().start());
        let Some(idx) = state.current_index else {
            return Ok(false);
        };
        let (from_char, to_char) = state.matches[idx];
        self.replace_char_range_atomic(from_char, to_char, replacement, EditKind::Replace)?;
        let new_caret = from_char + replacement.chars().count();
        state.recompute(self.rope(), new_caret);
        if let Some(next_idx) = state.current_index {
            let (ns, ne) = state.matches[next_idx];
            self.set_selection(Selection::new(ns, ne));
        }
        Ok(true)
    }

    pub fn page_search_replace_all(
        &mut self,
        query: &str,
        replacement: &str,
        case_sensitive: bool,
    ) -> Result<usize, EditorError> {
        let mut state = PageSearchState {
            active: true,
            query: query.to_owned(),
            replacement: replacement.to_owned(),
            case_sensitive,
            matches: Vec::new(),
            current_index: None,
        };
        state.recompute(self.rope(), 0);
        let count = state.matches.len();
        if count == 0 {
            return Ok(0);
        }
        for &(from_char, to_char) in state.matches.iter().rev() {
            self.replace_char_range_atomic(from_char, to_char, replacement, EditKind::Replace)?;
        }
        Ok(count)
    }

    /// Toggles inline Markdown formatting (`**` for bold, `*` for italic, `~~` for strikethrough,
    /// `` ` `` for inline code) around the current selection.
    pub fn toggle_inline_wrap(&mut self, marker: &str) -> Result<(), EditorError> {
        let (from_char, to_char) = self.selection().range();
        let m_chars = marker.chars().count();
        let selected = self.rope().slice_chars(from_char, to_char);

        // Case 1: Selected text itself starts and ends with `marker`
        if selected.chars().count() >= m_chars * 2
            && selected.starts_with(marker)
            && selected.ends_with(marker)
        {
            let inner: String = selected
                .chars()
                .skip(m_chars)
                .take(selected.chars().count() - m_chars * 2)
                .collect();
            let inner_len = inner.chars().count();
            self.replace_char_range_atomic(from_char, to_char, &inner, EditKind::Replace)?;
            self.set_selection(Selection::new(from_char, from_char + inner_len));
            return Ok(());
        }

        // Case 2: Surrounding characters right outside `[from_char..to_char]` are `marker`
        if from_char >= m_chars && to_char + m_chars <= self.rope().len_chars() {
            let before = self.rope().slice_chars(from_char - m_chars, from_char);
            let after = self.rope().slice_chars(to_char, to_char + m_chars);
            if before == marker && after == marker {
                self.replace_char_range_atomic(
                    from_char - m_chars,
                    to_char + m_chars,
                    &selected,
                    EditKind::Replace,
                )?;
                let new_start = from_char - m_chars;
                self.set_selection(Selection::new(
                    new_start,
                    new_start + selected.chars().count(),
                ));
                return Ok(());
            }
        }

        // Case 3: Wrap selection in `marker`
        let wrapped = format!("{marker}{selected}{marker}");
        self.replace_char_range_atomic(from_char, to_char, &wrapped, EditKind::Replace)?;
        let inner_start = from_char + m_chars;
        let inner_end = inner_start + selected.chars().count();
        self.set_selection(Selection::new(inner_start, inner_end));
        Ok(())
    }

    /// Inserts typed text while applying `editor.smart_dashes` (`--` -> `—`) when enabled.
    pub fn insert_text_with_typography(&mut self, text: &str) -> Result<(), EditorError> {
        let cfg = self.core().settings.get_config();
        if cfg.editor.smart_dashes && text == "-" && self.selection().is_empty() {
            let caret = self.selection().head;
            if caret > 0 && self.rope().char_at(caret - 1) == Some('-') {
                let prev2 = caret.checked_sub(2).and_then(|i| self.rope().char_at(i));
                // Convert `--` to `—` only when not part of `---` (YAML frontmatter / HR / table delimiter)
                if prev2 != Some('-') {
                    return self.replace_char_range_atomic(caret - 1, caret, "—", EditKind::Replace);
                }
            }
        }
        self.handle_key(super::editor::KeyCommand::InsertText(text.to_owned()))
    }

    /// Pastes text with `editor.auto_link_title` support: when pasting an HTTP(S) URL,
    /// either wraps the current selection as `[Selection](url)` or fetches the page title.
    pub fn paste_with_auto_link_title(&mut self, pasted: &str) -> Result<(), EditorError> {
        let trimmed = pasted.trim();
        let is_http_url = (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && !trimmed.chars().any(char::is_whitespace);

        if is_http_url && self.core().settings.get_config().editor.auto_link_title {
            let (from_char, to_char) = self.selection().range();
            if from_char < to_char {
                let label = self.rope().slice_chars(from_char, to_char);
                let md_link = format!("[{label}]({trimmed})");
                return self.replace_char_range_atomic(from_char, to_char, &md_link, EditKind::Paste);
            } else {
                let fetched_title = page_title(trimmed);
                let md_link = format!("[{fetched_title}]({trimmed})");
                return self.replace_char_range_atomic(from_char, to_char, &md_link, EditKind::Paste);
            }
        }
        self.handle_key(super::editor::KeyCommand::Paste(pasted.to_owned()))
    }

    /// Checks whether the cursor is inside an active `[[...` wiki-link trigger and queries
    /// `SearchService::suggest_notes` for autocomplete candidates.
    pub fn wiki_autocomplete(&self, workspace_path: &Path) -> Option<WikiAutocompletePopup> {
        let cfg = self.core().settings.get_config();
        if !cfg.editor.link_suggest || !self.selection().is_empty() {
            return None;
        }
        let caret = self.selection().head;
        let lc = self.rope().char_to_line_col(caret);
        let (line_start, line_end, _) = self.rope().line_char_bounds(lc.line);
        let before = self.rope().slice_chars(line_start, caret);
        let open_byte = before.rfind("[[")?;
        if before[open_byte + 2..].contains("]]") {
            return None;
        }
        let raw_inside = &before[open_byte + 2..];
        let query_part = raw_inside.split('|').next().unwrap_or(raw_inside).trim();
        if (query_part.chars().count() as u32) < cfg.editor.link_suggest_min_chars {
            return None;
        }
        let after = self.rope().slice_chars(caret, line_end);
        let has_closing = after.starts_with("]]");
        let query_start_char = line_start + before[..open_byte + 2].chars().count();

        let suggestions = self
            .core()
            .search
            .suggest_notes(&workspace_path.to_string_lossy(), query_part, 12)
            .unwrap_or_default();

        Some(WikiAutocompletePopup {
            query_char_range: (query_start_char, caret),
            has_closing_brackets: has_closing,
            query: query_part.to_owned(),
            suggestions,
        })
    }

    /// Completes a `[[WikiLink]]` using the chosen suggestion title and optional alias.
    pub fn accept_wiki_autocomplete(
        &mut self,
        popup: &WikiAutocompletePopup,
        chosen_title: &str,
        alias: Option<&str>,
    ) -> Result<(), EditorError> {
        let inner = match alias {
            Some(a) if !a.trim().is_empty() && a.trim() != chosen_title => {
                format!("{chosen_title}|{}", a.trim())
            }
            _ => chosen_title.to_owned(),
        };
        let suffix = if popup.has_closing_brackets { "" } else { "]]" };
        let replacement = format!("{inner}{suffix}");
        self.replace_char_range_atomic(
            popup.query_char_range.0,
            popup.query_char_range.1,
            &replacement,
            EditKind::Replace,
        )?;
        if popup.has_closing_brackets {
            let new_caret = (self.selection().head + 2).min(self.rope().len_chars());
            self.set_selection(Selection::caret(new_caret));
        }
        Ok(())
    }
}

fn collect_matches_in_slice(
    haystack: &str,
    needle_folded: &str,
    case_sensitive: bool,
    q_chars: usize,
    base_char_offset: usize,
    max_start_char_exclusive: Option<usize>,
    out: &mut Vec<(usize, usize)>,
) {
    if case_sensitive {
        let mut search_byte = 0;
        let mut cur_byte = 0;
        let mut cur_char = 0;
        while let Some(rel) = haystack[search_byte..].find(needle_folded) {
            let match_byte = search_byte + rel;
            cur_char += haystack[cur_byte..match_byte].chars().count();
            cur_byte = match_byte;
            if max_start_char_exclusive.is_some_and(|limit| cur_char >= limit) {
                break;
            }
            let abs_start = base_char_offset + cur_char;
            let abs_end = abs_start + q_chars;
            if out.last().is_none_or(|&(_, prev_end)| abs_start >= prev_end) {
                out.push((abs_start, abs_end));
            }
            let step_bytes = needle_folded.len().max(1);
            search_byte = match_byte + step_bytes;
        }
    } else {
        let folded_haystack: String = haystack.chars().flat_map(char::to_lowercase).collect();
        let mut search_byte = 0;
        let mut cur_byte = 0;
        let mut cur_char = 0;
        while let Some(rel) = folded_haystack[search_byte..].find(needle_folded) {
            let match_byte = search_byte + rel;
            cur_char += folded_haystack[cur_byte..match_byte].chars().count();
            cur_byte = match_byte;
            if max_start_char_exclusive.is_some_and(|limit| cur_char >= limit) {
                break;
            }
            let abs_start = base_char_offset + cur_char;
            let abs_end = abs_start + q_chars;
            if out.last().is_none_or(|&(_, prev_end)| abs_start >= prev_end) {
                out.push((abs_start, abs_end));
            }
            let step_bytes = needle_folded.len().max(1);
            search_byte = match_byte + step_bytes;
        }
    }
}
