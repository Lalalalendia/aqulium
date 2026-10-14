//! Stage 4 of Aquilum Native (Strangler Fig Migration):
//! Interactive Markdown Tables, Task Checkbox Toggling, WikiLink/ReaderQuote
//! Navigation, and Native Dataview Query Execution.

use super::editor::{EditorError, NativeEditorSession};
use super::history::EditKind;
use super::rope::{LineCol, RopeBuffer};
use super::selection::Selection;
use crate::app_core::Core;
use crate::documents::session::SYSTEM_CLIENT;
use crate::history::Source;
use crate::search::dataview::QueryOutput;
use crate::search::error::SearchError;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnAlignment {
    None,
    Left,
    Center,
    Right,
}

impl ColumnAlignment {
    pub fn from_delimiter_cell(cell: &str) -> Self {
        let trimmed = cell.trim();
        let left = trimmed.starts_with(':');
        let right = trimmed.ends_with(':');
        match (left, right) {
            (true, true) => Self::Center,
            (true, false) => Self::Left,
            (false, true) => Self::Right,
            (false, false) => Self::None,
        }
    }

    pub fn format_delimiter(self, width: usize) -> String {
        let w = width.max(3);
        match self {
            Self::None => "-".repeat(w),
            Self::Left => format!(":{}", "-".repeat(w.saturating_sub(1))),
            Self::Right => format!("{}:", "-".repeat(w.saturating_sub(1))),
            Self::Center => format!(":{}:", "-".repeat(w.saturating_sub(2).max(1))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownTable {
    pub start_line: usize,
    pub end_line: usize,
    pub start_char: usize,
    pub end_char: usize,
    pub headers: Vec<String>,
    pub alignments: Vec<ColumnAlignment>,
    pub rows: Vec<Vec<String>>,
}

impl MarkdownTable {
    pub fn col_count(&self) -> usize {
        self.headers.len()
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Formats the table into canonical aligned Markdown pipe syntax.
    pub fn format(&self) -> String {
        let cols = self.headers.len().max(1);
        let mut widths = vec![3usize; cols];
        for (c, h) in self.headers.iter().enumerate() {
            widths[c] = widths[c].max(h.trim().chars().count());
        }
        for row in &self.rows {
            for (c, cell) in row.iter().enumerate().take(cols) {
                widths[c] = widths[c].max(cell.trim().chars().count());
            }
        }

        let mut out = String::new();
        // Header row
        out.push('|');
        for c in 0..cols {
            let text = self.headers.get(c).map(|s| s.trim()).unwrap_or("");
            let pad = widths[c].saturating_sub(text.chars().count());
            out.push(' ');
            out.push_str(text);
            for _ in 0..pad {
                out.push(' ');
            }
            out.push_str(" |");
        }
        out.push('\n');

        // Alignment delimiter row
        out.push('|');
        for c in 0..cols {
            let align = self.alignments.get(c).copied().unwrap_or(ColumnAlignment::None);
            out.push(' ');
            out.push_str(&align.format_delimiter(widths[c]));
            out.push_str(" |");
        }

        // Data rows
        for row in &self.rows {
            out.push('\n');
            out.push('|');
            for c in 0..cols {
                let text = row.get(c).map(|s| s.trim()).unwrap_or("");
                let pad = widths[c].saturating_sub(text.chars().count());
                out.push(' ');
                out.push_str(text);
                for _ in 0..pad {
                    out.push(' ');
                }
                out.push_str(" |");
            }
        }
        out
    }
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() >= 2
}

fn is_delimiter_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|c| {
            let t = c.trim().trim_matches(':');
            !t.is_empty() && t.chars().all(|ch| ch == '-')
        })
}

fn split_pipe_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed
        .strip_prefix('|')
        .unwrap_or(trimmed)
        .strip_suffix('|')
        .unwrap_or(trimmed);
    inner.split('|').map(|s| s.trim().to_owned()).collect()
}

/// Parses a Markdown pipe table surrounding `cursor_line` in `rope`.
pub fn parse_table_at_line(rope: &RopeBuffer, cursor_line: usize) -> Option<MarkdownTable> {
    let total_lines = rope.len_lines();
    if cursor_line >= total_lines {
        return None;
    }
    let (l_start, l_end, _) = rope.line_char_bounds(cursor_line);
    if !is_table_line(&rope.slice_chars(l_start, l_end)) {
        return None;
    }

    let mut start_line = cursor_line;
    while start_line > 0 {
        let (s, e, _) = rope.line_char_bounds(start_line - 1);
        if is_table_line(&rope.slice_chars(s, e)) {
            start_line -= 1;
        } else {
            break;
        }
    }

    let mut end_line = cursor_line;
    while end_line + 1 < total_lines {
        let (s, e, _) = rope.line_char_bounds(end_line + 1);
        if is_table_line(&rope.slice_chars(s, e)) {
            end_line += 1;
        } else {
            break;
        }
    }

    if end_line < start_line + 1 {
        return None;
    }

    let (start_char, _, _) = rope.line_char_bounds(start_line);
    let (_, end_char, _) = rope.line_char_bounds(end_line);

    let mut raw_rows = Vec::with_capacity(end_line - start_line + 1);
    for line_idx in start_line..=end_line {
        let (s, e, _) = rope.line_char_bounds(line_idx);
        raw_rows.push(split_pipe_cells(&rope.slice_chars(s, e)));
    }

    if raw_rows.len() < 2 || !is_delimiter_row(&raw_rows[1]) {
        return None;
    }

    let headers = raw_rows[0].clone();
    let cols = headers.len();
    let alignments: Vec<ColumnAlignment> = (0..cols)
        .map(|c| {
            raw_rows[1]
                .get(c)
                .map(|cell| ColumnAlignment::from_delimiter_cell(cell))
                .unwrap_or(ColumnAlignment::None)
        })
        .collect();

    let mut rows = Vec::with_capacity(raw_rows.len().saturating_sub(2));
    for r in raw_rows.into_iter().skip(2) {
        let mut normalized = vec![String::new(); cols];
        for (c, val) in r.into_iter().enumerate().take(cols) {
            normalized[c] = val;
        }
        rows.push(normalized);
    }

    Some(MarkdownTable {
        start_line,
        end_line,
        start_char,
        end_char,
        headers,
        alignments,
        rows,
    })
}

fn current_table_cell_coords(rope: &RopeBuffer, selection: Selection, table: &MarkdownTable) -> (usize, usize) {
    let lc = rope.char_to_line_col(selection.head);
    let (line_start, _, _) = rope.line_char_bounds(lc.line);
    let prefix = rope.slice_chars(line_start, selection.head);
    let pipes_before = prefix.chars().filter(|&c| c == '|').count();
    let col_idx = pipes_before.saturating_sub(1).min(table.col_count().saturating_sub(1));
    let row_offset = lc.line.saturating_sub(table.start_line);
    (row_offset, col_idx)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InteractiveLinkTarget {
    WikiNote {
        raw_target: String,
        alias: Option<String>,
        resolved_path: Option<String>,
    },
    ReaderQuote {
        book_path: String,
        cfi: String,
    },
}

#[derive(Clone, Debug)]
pub struct EvaluatedDataviewBlock {
    pub start_line: usize,
    pub end_line: usize,
    pub query: String,
    pub result: Result<QueryOutput, String>,
}

impl NativeEditorSession {
    /// Formats or mutates a Markdown table at the current cursor position.
    pub fn table_insert_row_below(&mut self) -> Result<bool, EditorError> {
        let lc = self.rope().char_to_line_col(self.selection().head);
        let Some(mut table) = parse_table_at_line(self.rope(), lc.line) else {
            return Ok(false);
        };
        let (row_offset, _) = current_table_cell_coords(self.rope(), self.selection(), &table);
        let insert_idx = if row_offset <= 1 {
            0
        } else {
            (row_offset - 1).min(table.rows.len())
        };
        table.rows.insert(insert_idx, vec![String::new(); table.col_count()]);
        let formatted = table.format();
        self.replace_char_range_atomic(table.start_char, table.end_char, &formatted, EditKind::Replace)?;
        Ok(true)
    }

    pub fn table_delete_current_row(&mut self) -> Result<bool, EditorError> {
        let lc = self.rope().char_to_line_col(self.selection().head);
        let Some(mut table) = parse_table_at_line(self.rope(), lc.line) else {
            return Ok(false);
        };
        let (row_offset, _) = current_table_cell_coords(self.rope(), self.selection(), &table);
        if row_offset < 2 || table.rows.is_empty() {
            return Ok(false);
        }
        let data_row_idx = (row_offset - 2).min(table.rows.len() - 1);
        table.rows.remove(data_row_idx);
        let formatted = table.format();
        self.replace_char_range_atomic(table.start_char, table.end_char, &formatted, EditKind::Replace)?;
        Ok(true)
    }

    pub fn table_insert_column_after(&mut self) -> Result<bool, EditorError> {
        let lc = self.rope().char_to_line_col(self.selection().head);
        let Some(mut table) = parse_table_at_line(self.rope(), lc.line) else {
            return Ok(false);
        };
        let (_, col_idx) = current_table_cell_coords(self.rope(), self.selection(), &table);
        let insert_col = (col_idx + 1).min(table.col_count());
        table.headers.insert(insert_col, format!("Col {}", insert_col + 1));
        table.alignments.insert(insert_col, ColumnAlignment::None);
        for row in &mut table.rows {
            row.insert(insert_col, String::new());
        }
        let formatted = table.format();
        self.replace_char_range_atomic(table.start_char, table.end_char, &formatted, EditKind::Replace)?;
        Ok(true)
    }

    pub fn table_delete_current_column(&mut self) -> Result<bool, EditorError> {
        let lc = self.rope().char_to_line_col(self.selection().head);
        let Some(mut table) = parse_table_at_line(self.rope(), lc.line) else {
            return Ok(false);
        };
        if table.col_count() <= 1 {
            return Ok(false);
        }
        let (_, col_idx) = current_table_cell_coords(self.rope(), self.selection(), &table);
        table.headers.remove(col_idx);
        table.alignments.remove(col_idx);
        for row in &mut table.rows {
            if col_idx < row.len() {
                row.remove(col_idx);
            }
        }
        let formatted = table.format();
        self.replace_char_range_atomic(table.start_char, table.end_char, &formatted, EditKind::Replace)?;
        Ok(true)
    }

    pub fn table_set_column_alignment(&mut self, align: ColumnAlignment) -> Result<bool, EditorError> {
        let lc = self.rope().char_to_line_col(self.selection().head);
        let Some(mut table) = parse_table_at_line(self.rope(), lc.line) else {
            return Ok(false);
        };
        let (_, col_idx) = current_table_cell_coords(self.rope(), self.selection(), &table);
        if let Some(slot) = table.alignments.get_mut(col_idx) {
            *slot = align;
        }
        let formatted = table.format();
        self.replace_char_range_atomic(table.start_char, table.end_char, &formatted, EditKind::Replace)?;
        Ok(true)
    }

    /// Moves the cursor to the next (`forward = true`) or previous (`forward = false`) cell
    /// in a Markdown table. When pressing Tab on the last cell of the last row, automatically
    /// appends a new row.
    pub fn table_navigate_cell(&mut self, forward: bool) -> Result<bool, EditorError> {
        let lc = self.rope().char_to_line_col(self.selection().head);
        let Some(mut table) = parse_table_at_line(self.rope(), lc.line) else {
            return Ok(false);
        };
        let (row_offset, col_idx) = current_table_cell_coords(self.rope(), self.selection(), &table);
        let cols = table.col_count();
        let current_logical_row = if row_offset == 0 {
            0
        } else {
            row_offset.saturating_sub(1)
        };
        let total_logical_rows = 1 + table.rows.len();

        let (target_logical_row, target_col) = if forward {
            if col_idx + 1 < cols {
                (current_logical_row, col_idx + 1)
            } else if current_logical_row + 1 < total_logical_rows {
                (current_logical_row + 1, 0)
            } else {
                // Append a new row at the bottom of the table
                table.rows.push(vec![String::new(); cols]);
                (current_logical_row + 1, 0)
            }
        } else if col_idx > 0 {
            (current_logical_row, col_idx - 1)
        } else if current_logical_row > 0 {
            (current_logical_row - 1, cols.saturating_sub(1))
        } else {
            (0, 0)
        };

        let formatted = table.format();
        self.replace_char_range_atomic(table.start_char, table.end_char, &formatted, EditKind::Replace)?;

        // Position caret inside `(target_logical_row, target_col)`
        let physical_line = if target_logical_row == 0 {
            table.start_line
        } else {
            table.start_line + 1 + target_logical_row
        };
        let (line_start, line_end, _) = self.rope().line_char_bounds(physical_line);
        let line_str = self.rope().slice_chars(line_start, line_end);
        let mut pipe_seen = 0;
        let mut caret_col = 0;
        for (ch_idx, ch) in line_str.chars().enumerate() {
            if ch == '|' {
                if pipe_seen == target_col {
                    caret_col = (ch_idx + 2).min(line_str.chars().count());
                    break;
                }
                pipe_seen += 1;
            }
        }
        self.set_selection(Selection::caret(line_start + caret_col));
        Ok(true)
    }

    /// Toggles a Markdown task checkbox (`- [ ] ` <-> `- [x] `) on `target_line` (0-based).
    pub fn toggle_task_at_line(&mut self, target_line: usize) -> Result<bool, EditorError> {
        if target_line >= self.rope().len_lines() {
            return Ok(false);
        }
        let (line_start, line_end, _) = self.rope().line_char_bounds(target_line);
        let line = self.rope().slice_chars(line_start, line_end);
        let indent_chars = line.chars().take_while(|c| c.is_whitespace()).count();
        let rest: String = line.chars().skip(indent_chars).collect();

        for (unchecked, checked) in [("- [ ] ", "- [x] "), ("* [ ] ", "* [x] ")] {
            if rest.starts_with(unchecked) {
                let marker_start = line_start + indent_chars;
                let marker_end = marker_start + unchecked.chars().count();
                self.replace_char_range_atomic(marker_start, marker_end, checked, EditKind::Replace)?;
                return Ok(true);
            }
            if rest.starts_with(checked) || rest.starts_with(&checked.replace('x', "X")) {
                let marker_start = line_start + indent_chars;
                let marker_end = marker_start + checked.chars().count();
                self.replace_char_range_atomic(marker_start, marker_end, unchecked, EditKind::Replace)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Resolves a `[[WikiLink|Alias]]` or `aquilum-reader://...` link under the current cursor.
    pub fn resolve_link_at_cursor(&self, workspace_path: &Path) -> Option<InteractiveLinkTarget> {
        let lc = self.rope().char_to_line_col(self.selection().head);
        let (line_start, line_end, _) = self.rope().line_char_bounds(lc.line);
        let line = self.rope().slice_chars(line_start, line_end);
        let col = lc.column_chars;

        // Check for aquilum-reader:// link on this line
        if let Some(pos) = line.find("aquilum-reader://") {
            let tail = &line[pos..];
            let end_rel = tail.find([')', ' ', '"', '>']).unwrap_or(tail.len());
            let url = &tail[..end_rel];
            if let Some((book_path, cfi)) = crate::native_reader::parse_reader_quote_url(url) {
                return Some(InteractiveLinkTarget::ReaderQuote { book_path, cfi });
            }
        }

        // Check for [[WikiLink]] containing `col` (or on the line)
        let mut search_from = 0;
        while let Some(open_rel) = line[search_from..].find("[[") {
            let open_byte = search_from + open_rel;
            let after_open = open_byte + 2;
            let Some(close_rel) = line[after_open..].find("]]") else { break };
            let close_byte = after_open + close_rel + 2;
            let start_char = line[..open_byte].chars().count();
            let end_char = start_char + line[open_byte..close_byte].chars().count();

            if col >= start_char && col <= end_char {
                let inner = &line[after_open..after_open + close_rel];
                let (raw_target, alias) = match inner.split_once('|') {
                    Some((t, a)) => (t.trim().to_owned(), Some(a.trim().to_owned())),
                    None => (inner.trim().to_owned(), None),
                };
                let resolved_path = self
                    .core()
                    .search
                    .resolve_wiki_links(
                        &workspace_path.to_string_lossy(),
                        &self.path().to_string_lossy(),
                        &[raw_target.clone()],
                    )
                    .ok()
                    .and_then(|mut res| res.paths.pop().flatten());
                return Some(InteractiveLinkTarget::WikiNote {
                    raw_target,
                    alias,
                    resolved_path,
                });
            }
            search_from = close_byte;
        }
        None
    }

    /// Scans the active document for fenced ```dataview blocks and executes each query
    /// directly against `aquilum_core::search::SearchService::run_dataview_query`.
    pub fn evaluate_dataview_blocks(
        &self,
        workspace_path: &Path,
        tz_offset_minutes: i64,
    ) -> Vec<EvaluatedDataviewBlock> {
        // V2 O(1) Presence Fastpath: zero allocations and zero scanning when no dataview fence exists!
        if !self.rope().has_dataview_fence() {
            return Vec::new();
        }
        let full_text = self.rope().to_string_full();
        let mut blocks = Vec::new();
        let mut in_dataview = false;
        let mut start_line = 0;
        let mut query_buf = String::new();

        for (line_idx, line) in full_text.lines().enumerate() {
            let trimmed = line.trim();
            if !in_dataview {
                if trimmed.eq_ignore_ascii_case("```dataview") {
                    in_dataview = true;
                    start_line = line_idx;
                    query_buf.clear();
                }
            } else if trimmed == "```" {
                in_dataview = false;
                let query = query_buf.trim().to_owned();
                let result = self
                    .core()
                    .search
                    .run_dataview_query(
                        &workspace_path.to_string_lossy(),
                        &self.path().to_string_lossy(),
                        &query,
                        tz_offset_minutes,
                    )
                    .map_err(|e: SearchError| format!("{e:?}"));
                blocks.push(EvaluatedDataviewBlock {
                    start_line,
                    end_line: line_idx,
                    query,
                    result,
                });
            } else {
                if !query_buf.is_empty() {
                    query_buf.push('\n');
                }
                query_buf.push_str(line);
            }
        }
        blocks
    }
}

/// Toggles a task checkbox directly from a Dataview `TASK` / `TABLE` query result cell
/// (`CellPart::Check { done, target, line }`) and updates both disk and search index.
pub fn toggle_dataview_task(
    core: &Core,
    workspace_path: &Path,
    target_relative: &str,
    line_1based: usize,
) -> Result<bool, EditorError> {
    let target_path = if Path::new(target_relative).is_absolute() {
        Path::new(target_relative).to_path_buf()
    } else {
        workspace_path.join(target_relative)
    };
    let opened = core.documents.read(core, &target_path)?;
    let mut rope = RopeBuffer::from_text(&opened.text);
    let line_0based = line_1based.saturating_sub(1);
    if line_0based >= rope.len_lines() {
        return Ok(false);
    }
    let (line_start, line_end, _) = rope.line_char_bounds(line_0based);
    let line = rope.slice_chars(line_start, line_end);
    let indent_chars = line.chars().take_while(|c| c.is_whitespace()).count();
    let rest: String = line.chars().skip(indent_chars).collect();

    for (unchecked, checked) in [("- [ ] ", "- [x] "), ("* [ ] ", "* [x] ")] {
        if rest.starts_with(unchecked) {
            let m_start = line_start + indent_chars;
            let m_end = m_start + unchecked.chars().count();
            rope.replace_char_range(m_start, m_end, checked);
            let updated = rope.to_string_full();
            core.documents
                .replace_text(core, &target_path, &updated, SYSTEM_CLIENT, Source::Me, None)?;
            return Ok(true);
        }
        if rest.starts_with(checked) || rest.starts_with(&checked.replace('x', "X")) {
            let m_start = line_start + indent_chars;
            let m_end = m_start + checked.chars().count();
            rope.replace_char_range(m_start, m_end, unchecked);
            let updated = rope.to_string_full();
            core.documents
                .replace_text(core, &target_path, &updated, SYSTEM_CLIENT, Source::Me, None)?;
            return Ok(true);
        }
    }
    Ok(false)
}
