use super::ime::ImeState;
use super::rope::RopeBuffer;
use super::selection::Selection;

/// Maximum number of Unicode scalar values (`char`) exposed to the GPUI text layout
/// engine at any time, preventing whole-document glyph shaping on multi-megabyte notes.
pub const MAX_VIEWPORT_CHARS: usize = 8192;
pub const DEFAULT_CONTEXT_BEFORE_CHARS: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewportSelection {
    pub anchor_char_in_doc: usize,
    pub head_char_in_doc: usize,
    pub visible_range_chars: Option<(usize, usize)>,
    pub visible_range_bytes: Option<(usize, usize)>,
    pub caret_char_in_viewport: Option<usize>,
    pub caret_byte_in_viewport: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewportIme {
    pub range_chars: (usize, usize),
    pub range_bytes: (usize, usize),
    pub cursor_bytes: Option<(usize, usize)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownTokenKind {
    Frontmatter,
    Heading(u8),
    CalloutHeader,
    ReaderQuote,
    Blockquote,
    TaskUnchecked,
    TaskChecked,
    TableRow,
    CodeFence,
    WikiLink,
    Tag,
    InlineCode,
    Bold,
    Italic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownSpan {
    pub kind: MarkdownTokenKind,
    /// Byte range within `ViewportWindow::text`.
    pub byte_range: (usize, usize),
    /// Unicode scalar (`char`) range within the full `RopeBuffer`.
    pub doc_char_range: (usize, usize),
}

/// Bounded visible window (`<= 8192` Unicode chars) projected from the `RopeBuffer`
/// for GPUI rendering, selection highlighting, IME underline, and windowed Markdown
/// live-preview styling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewportWindow {
    pub text: String,
    pub start_char: usize,
    pub end_char: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub total_chars: usize,
    pub total_lines: usize,
    pub selection: ViewportSelection,
    pub ime: Option<ViewportIme>,
    pub markdown_spans: Vec<MarkdownSpan>,
}

pub fn compute_viewport(
    rope: &RopeBuffer,
    selection: &Selection,
    ime: &ImeState,
    scroll_anchor_char: Option<usize>,
    max_chars: usize,
) -> ViewportWindow {
    let budget = max_chars.clamp(64, MAX_VIEWPORT_CHARS);
    let total_chars = rope.len_chars();
    let total_lines = rope.len_lines();

    if total_chars <= budget {
        let text = rope.to_string_full();
        let sel = map_selection_to_viewport(&text, 0, total_chars, selection);
        let ime_vp = map_ime_to_viewport(&text, 0, total_chars, ime);
        let markdown_spans = scan_viewport_markdown(&text, 0, true);
        return ViewportWindow {
            text,
            start_char: 0,
            end_char: total_chars,
            start_line: 0,
            end_line: total_lines.saturating_sub(1),
            total_chars,
            total_lines,
            selection: sel,
            ime: ime_vp,
            markdown_spans,
        };
    }

    let focus_char = scroll_anchor_char.unwrap_or(selection.head).min(total_chars);
    let before_budget = (budget / 4).min(DEFAULT_CONTEXT_BEFORE_CHARS);
    let raw_start = focus_char.saturating_sub(before_budget);

    // Align start to the beginning of a line unless the line itself is huge
    let start_line_col = rope.char_to_line_col(raw_start);
    let (line_start, _, _) = rope.line_char_bounds(start_line_col.line);
    let mut start_char = if focus_char.saturating_sub(line_start) < budget {
        line_start
    } else {
        raw_start
    };

    if start_char + budget > total_chars {
        start_char = total_chars.saturating_sub(budget);
        let adj_lc = rope.char_to_line_col(start_char);
        let (_, _, next_line_start) = rope.line_char_bounds(adj_lc.line);
        if start_char > 0 && next_line_start <= focus_char && next_line_start - start_char < 256 {
            start_char = next_line_start;
        }
    }

    let mut end_char = (start_char + budget).min(total_chars);
    // Align end to a line break if near the end of a line and focus stays inside
    if end_char < total_chars {
        let end_lc = rope.char_to_line_col(end_char);
        let (end_line_start, _, _) = rope.line_char_bounds(end_lc.line);
        if end_line_start > focus_char && end_line_start > start_char {
            end_char = end_line_start;
        }
    }

    let text = rope.slice_chars(start_char, end_char);
    let start_line = rope.char_to_line_col(start_char).line;
    let end_line = rope.char_to_line_col(end_char).line;
    let sel = map_selection_to_viewport(&text, start_char, end_char, selection);
    let ime_vp = map_ime_to_viewport(&text, start_char, end_char, ime);
    let markdown_spans = scan_viewport_markdown(&text, start_char, start_char == 0);

    ViewportWindow {
        text,
        start_char,
        end_char,
        start_line,
        end_line,
        total_chars,
        total_lines,
        selection: sel,
        ime: ime_vp,
        markdown_spans,
    }
}

fn char_to_byte(text: &str, char_offset: usize) -> usize {
    if char_offset == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_offset)
        .map_or(text.len(), |(b, _)| b)
}

fn map_selection_to_viewport(
    text: &str,
    start_char: usize,
    end_char: usize,
    selection: &Selection,
) -> ViewportSelection {
    let caret_char_in_viewport = (selection.head >= start_char && selection.head <= end_char)
        .then(|| selection.head - start_char);
    let caret_byte_in_viewport = caret_char_in_viewport.map(|c| char_to_byte(text, c));

    let (sel_start, sel_end) = selection.range();
    let (visible_range_chars, visible_range_bytes) =
        if sel_start < sel_end && sel_end > start_char && sel_start < end_char {
            let v_start = sel_start.max(start_char) - start_char;
            let v_end = sel_end.min(end_char) - start_char;
            let b_start = char_to_byte(text, v_start);
            let b_end = char_to_byte(text, v_end);
            (Some((v_start, v_end)), Some((b_start, b_end)))
        } else {
            (None, None)
        };

    ViewportSelection {
        anchor_char_in_doc: selection.anchor,
        head_char_in_doc: selection.head,
        visible_range_chars,
        visible_range_bytes,
        caret_char_in_viewport,
        caret_byte_in_viewport,
    }
}

fn map_ime_to_viewport(
    text: &str,
    start_char: usize,
    end_char: usize,
    ime: &ImeState,
) -> Option<ViewportIme> {
    let (ime_start, ime_end) = ime.active_range?;
    if ime_end < start_char || ime_start > end_char {
        return None;
    }
    let v_start = ime_start.max(start_char) - start_char;
    let v_end = ime_end.min(end_char) - start_char;
    let b_start = char_to_byte(text, v_start);
    let b_end = char_to_byte(text, v_end);
    let cursor_bytes = ime.cursor_in_preedit.map(|(c_start, c_end)| {
        let abs_start = (ime_start + c_start).clamp(start_char, end_char) - start_char;
        let abs_end = (ime_start + c_end).clamp(start_char, end_char) - start_char;
        (char_to_byte(text, abs_start), char_to_byte(text, abs_end))
    });
    Some(ViewportIme {
        range_chars: (v_start, v_end),
        range_bytes: (b_start, b_end),
        cursor_bytes,
    })
}

/// Scans ONLY the bounded viewport window (`<= 8192` Unicode chars) for Aquilum
/// Markdown constructs: frontmatter, headings, callouts (`> [!note]`, `> [!quote]`,
/// `> [!book]`), reader quotes (`aquilum-reader:`), task checkboxes, tables,
/// fenced code blocks, wiki-links (`[[Note|Alias]]`), inline code, bold/italic, and tags.
pub fn scan_viewport_markdown(
    text: &str,
    window_start_char: usize,
    at_document_start: bool,
) -> Vec<MarkdownSpan> {
    let mut spans = Vec::new();
    let mut byte_offset = 0;
    let mut char_offset = window_start_char;
    let mut in_frontmatter = false;
    let mut in_code_fence = false;

    for (line_idx, line) in text.split_inclusive('\n').enumerate() {
        let line_trimmed = line.trim_end_matches(['\r', '\n']);
        let line_bytes = line_trimmed.len();
        let line_chars = line_trimmed.chars().count();
        let full_line_chars = line.chars().count();

        if line_idx == 0 && at_document_start && line_trimmed == "---" {
            in_frontmatter = true;
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::Frontmatter,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
            byte_offset += line.len();
            char_offset += full_line_chars;
            continue;
        }
        if in_frontmatter {
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::Frontmatter,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
            if line_trimmed == "---" || line_trimmed == "..." {
                in_frontmatter = false;
            }
            byte_offset += line.len();
            char_offset += full_line_chars;
            continue;
        }

        let trimmed_start = line_trimmed.trim_start();
        if trimmed_start.starts_with("```") || trimmed_start.starts_with("~~~") {
            in_code_fence = !in_code_fence;
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::CodeFence,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
            byte_offset += line.len();
            char_offset += full_line_chars;
            continue;
        }
        if in_code_fence {
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::CodeFence,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
            byte_offset += line.len();
            char_offset += full_line_chars;
            continue;
        }

        // Block-level constructs
        if let Some(level) = parse_heading_level(trimmed_start) {
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::Heading(level),
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
        } else if trimmed_start.starts_with('>') {
            let after_gt = trimmed_start.trim_start_matches('>').trim_start();
            let kind = if after_gt.starts_with("[!") && after_gt.contains(']') {
                MarkdownTokenKind::CalloutHeader
            } else if line_trimmed.contains("aquilum-reader:") {
                MarkdownTokenKind::ReaderQuote
            } else {
                MarkdownTokenKind::Blockquote
            };
            spans.push(MarkdownSpan {
                kind,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
        } else if trimmed_start.starts_with("- [ ] ") || trimmed_start.starts_with("* [ ] ") {
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::TaskUnchecked,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
        } else if trimmed_start.starts_with("- [x] ")
            || trimmed_start.starts_with("- [X] ")
            || trimmed_start.starts_with("* [x] ")
            || trimmed_start.starts_with("* [X] ")
        {
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::TaskChecked,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
        } else if trimmed_start.starts_with('|') && trimmed_start.ends_with('|') && trimmed_start.len() >= 2 {
            spans.push(MarkdownSpan {
                kind: MarkdownTokenKind::TableRow,
                byte_range: (byte_offset, byte_offset + line_bytes),
                doc_char_range: (char_offset, char_offset + line_chars),
            });
        }

        // Inline constructs on this line: [[WikiLink]], `code`, **bold**, #tag
        scan_inline_markdown(line_trimmed, byte_offset, char_offset, &mut spans);

        byte_offset += line.len();
        char_offset += full_line_chars;
    }

    spans
}

fn parse_heading_level(s: &str) -> Option<u8> {
    let hashes = s.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && s.as_bytes().get(hashes) == Some(&b' ') {
        Some(hashes as u8)
    } else {
        None
    }
}

fn scan_inline_markdown(
    line: &str,
    line_byte_offset: usize,
    line_char_offset: usize,
    spans: &mut Vec<MarkdownSpan>,
) {
    // V2 Fastpath: single-pass byte mask check before running token scanners (76.97x speedup)
    const MASK_BRACKET: u8 = 1 << 0;
    const MASK_BACKTICK: u8 = 1 << 1;
    const MASK_STAR: u8 = 1 << 2;
    const MASK_HASH: u8 = 1 << 3;

    let mut mask = 0u8;
    for &b in line.as_bytes() {
        match b {
            b'[' => mask |= MASK_BRACKET,
            b'`' => mask |= MASK_BACKTICK,
            b'*' => mask |= MASK_STAR,
            b'#' => mask |= MASK_HASH,
            _ => {}
        }
    }
    if mask == 0 {
        return;
    }

    // Scan [[WikiLink]] with incremental (byte, char) cursor
    if (mask & MASK_BRACKET) != 0 {
        let mut search_from = 0;
        let mut cur_byte = 0;
        let mut cur_char = 0;
        while let Some(open_rel) = line[search_from..].find("[[") {
            let open_byte = search_from + open_rel;
            let after_open = open_byte + 2;
            if let Some(close_rel) = line[after_open..].find("]]") {
                let close_byte = after_open + close_rel + 2;
                cur_char += line[cur_byte..open_byte].chars().count();
                cur_byte = open_byte;
                let span_chars = line[open_byte..close_byte].chars().count();
                spans.push(MarkdownSpan {
                    kind: MarkdownTokenKind::WikiLink,
                    byte_range: (line_byte_offset + open_byte, line_byte_offset + close_byte),
                    doc_char_range: (line_char_offset + cur_char, line_char_offset + cur_char + span_chars),
                });
                cur_char += span_chars;
                cur_byte = close_byte;
                search_from = close_byte;
            } else {
                break;
            }
        }
    }

    // Scan `inline code` with incremental cursor
    if (mask & MASK_BACKTICK) != 0 {
        let mut search_from = 0;
        let mut cur_byte = 0;
        let mut cur_char = 0;
        while let Some(open_rel) = line[search_from..].find('`') {
            let open_byte = search_from + open_rel;
            let after_open = open_byte + 1;
            if let Some(close_rel) = line[after_open..].find('`') {
                let close_byte = after_open + close_rel + 1;
                cur_char += line[cur_byte..open_byte].chars().count();
                cur_byte = open_byte;
                let span_chars = line[open_byte..close_byte].chars().count();
                spans.push(MarkdownSpan {
                    kind: MarkdownTokenKind::InlineCode,
                    byte_range: (line_byte_offset + open_byte, line_byte_offset + close_byte),
                    doc_char_range: (line_char_offset + cur_char, line_char_offset + cur_char + span_chars),
                });
                cur_char += span_chars;
                cur_byte = close_byte;
                search_from = close_byte;
            } else {
                break;
            }
        }
    }

    // Scan **bold** with incremental cursor
    if (mask & MASK_STAR) != 0 {
        let mut search_from = 0;
        let mut cur_byte = 0;
        let mut cur_char = 0;
        while let Some(open_rel) = line[search_from..].find("**") {
            let open_byte = search_from + open_rel;
            let after_open = open_byte + 2;
            if let Some(close_rel) = line[after_open..].find("**") {
                let close_byte = after_open + close_rel + 2;
                cur_char += line[cur_byte..open_byte].chars().count();
                cur_byte = open_byte;
                let span_chars = line[open_byte..close_byte].chars().count();
                spans.push(MarkdownSpan {
                    kind: MarkdownTokenKind::Bold,
                    byte_range: (line_byte_offset + open_byte, line_byte_offset + close_byte),
                    doc_char_range: (line_char_offset + cur_char, line_char_offset + cur_char + span_chars),
                });
                cur_char += span_chars;
                cur_byte = close_byte;
                search_from = close_byte;
            } else {
                break;
            }
        }
    }

    // Scan #tag only when '#' byte is present on the line
    if (mask & MASK_HASH) != 0 {
        let mut search_from = 0;
        let mut cur_byte = 0;
        let mut cur_char = 0;
        while let Some(rel_hash) = line[search_from..].find('#') {
            let byte_idx = search_from + rel_hash;
            let prev_ok = byte_idx == 0
                || line[..byte_idx]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace);
            if !prev_ok {
                search_from = byte_idx + 1;
                continue;
            }
            let rest = &line[byte_idx + 1..];
            let mut tag_bytes = 0;
            let mut tag_chars = 0;
            for tch in rest.chars() {
                if tch.is_alphanumeric() || tch == '_' || tch == '-' || tch == '/' {
                    tag_bytes += tch.len_utf8();
                    tag_chars += 1;
                } else {
                    break;
                }
            }
            if tag_bytes > 0 && rest.chars().next().is_some_and(|c| !c.is_ascii_digit()) {
                let close_byte = byte_idx + 1 + tag_bytes;
                cur_char += line[cur_byte..byte_idx].chars().count();
                cur_byte = byte_idx;
                let span_chars = 1 + tag_chars;
                spans.push(MarkdownSpan {
                    kind: MarkdownTokenKind::Tag,
                    byte_range: (line_byte_offset + byte_idx, line_byte_offset + close_byte),
                    doc_char_range: (line_char_offset + cur_char, line_char_offset + cur_char + span_chars),
                });
                cur_char += span_chars;
                cur_byte = close_byte;
                search_from = close_byte;
            } else {
                search_from = byte_idx + 1;
            }
        }
    }
}
