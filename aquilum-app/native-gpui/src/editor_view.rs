use aquilum_core::app_core::Core;
use aquilum_core::native_editor::{
    EditorError, EditorMetrics, KeyCommand, MarkdownSpan, NativeEditorSession, Selection,
    ViewportIme, ViewportSelection, ViewportWindow, MAX_VIEWPORT_CHARS,
};
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;

/// High-level actions dispatched by GPUI keybindings or menu items into `GpuiRopeEditorView`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuiEditorAction {
    Key(KeyCommand),
    MouseDown { viewport_char_offset: usize, shift: bool },
    MouseDrag { viewport_char_offset: usize },
    ScrollLines { delta_lines: isize },
    SetViewportBudget { max_chars: usize },
}

/// Immutable frame projection handed to GPUI's `Render::render` pass.
/// Contains strictly at most `MAX_VIEWPORT_CHARS` (8,192 Unicode scalars).
#[derive(Clone, Debug)]
pub struct GpuiFrameState {
    pub title: String,
    pub dirty: bool,
    pub version: u64,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub viewport: ViewportWindow,
    pub metrics: EditorMetrics,
}

/// Trait matching GPUI's `EntityInputHandler` UTF-16 OS IME contract so Windows (TSF/IMM32),
/// macOS (NSTextInputClient), and Linux (IBus/Fcitx/Wayland text-input-v3) compose text
/// directly against the Rope + Viewport model without corrupting surrogate pairs.
pub trait GpuiImeBridge {
    fn text_for_range(&mut self, range_utf16: Range<usize>) -> Option<String>;
    fn selected_text_range(&mut self) -> Option<Range<usize>>;
    fn marked_text_range(&self) -> Option<Range<usize>>;
    fn unmark_text(&mut self);
    fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, text: &str);
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    );
}

pub struct GpuiRopeEditorView {
    session: NativeEditorSession,
    drag_anchor_doc_char: Option<usize>,
}

impl GpuiRopeEditorView {
    pub fn open(core: Arc<Core>, path: impl AsRef<Path>) -> Result<Self, EditorError> {
        let session = NativeEditorSession::open(core, path, "aquilum-gpui")?;
        Ok(Self {
            session,
            drag_anchor_doc_char: None,
        })
    }

    pub fn session(&self) -> &NativeEditorSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut NativeEditorSession {
        &mut self.session
    }

    pub fn dispatch(&mut self, action: GpuiEditorAction) -> Result<(), EditorError> {
        match action {
            GpuiEditorAction::Key(cmd) => self.session.handle_key(cmd),
            GpuiEditorAction::MouseDown {
                viewport_char_offset,
                shift,
            } => {
                let vp = self.session.viewport();
                let doc_char = (vp.start_char + viewport_char_offset).min(self.session.rope().len_chars());
                let anchor = if shift {
                    self.session.selection().anchor
                } else {
                    doc_char
                };
                self.drag_anchor_doc_char = Some(anchor);
                self.session.set_selection(Selection::new(anchor, doc_char));
                Ok(())
            }
            GpuiEditorAction::MouseDrag {
                viewport_char_offset,
            } => {
                let vp = self.session.viewport();
                let doc_char = (vp.start_char + viewport_char_offset).min(self.session.rope().len_chars());
                let anchor = self.drag_anchor_doc_char.unwrap_or(self.session.selection().anchor);
                self.session.set_selection(Selection::new(anchor, doc_char));
                Ok(())
            }
            GpuiEditorAction::ScrollLines { delta_lines } => {
                let vp = self.session.viewport();
                let target_line = if delta_lines < 0 {
                    vp.start_line.saturating_sub((-delta_lines) as usize)
                } else {
                    (vp.start_line + delta_lines as usize)
                        .min(self.session.rope().len_lines().saturating_sub(1))
                };
                self.session.scroll_to_line(target_line);
                Ok(())
            }
            GpuiEditorAction::SetViewportBudget { max_chars } => {
                self.session.set_viewport_budget_chars(max_chars.min(MAX_VIEWPORT_CHARS));
                Ok(())
            }
        }
    }

    /// Produces the bounded frame state (`<= 8192` Unicode chars) for GPUI rendering.
    pub fn render_frame(&self) -> GpuiFrameState {
        let viewport = self.session.viewport();
        let metrics = self.session.metrics();
        let lc = self.session.rope().char_to_line_col(self.session.selection().head);
        let title = self
            .session
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Aquilum".to_owned());

        GpuiFrameState {
            title,
            dirty: metrics.dirty,
            version: metrics.version,
            cursor_line: lc.line + 1,
            cursor_column: lc.column_chars + 1,
            viewport,
            metrics,
        }
    }

    pub fn viewport_selection(&self) -> ViewportSelection {
        self.session.viewport().selection
    }

    pub fn viewport_ime(&self) -> Option<ViewportIme> {
        self.session.viewport().ime
    }

    pub fn viewport_markdown_spans(&self) -> Vec<MarkdownSpan> {
        self.session.viewport().markdown_spans
    }
}

impl GpuiImeBridge for GpuiRopeEditorView {
    fn text_for_range(&mut self, range_utf16: Range<usize>) -> Option<String> {
        let start_char = self.session.rope().utf16_to_char(range_utf16.start);
        let end_char = self.session.rope().utf16_to_char(range_utf16.end);
        Some(self.session.rope().slice_chars(start_char, end_char))
    }

    fn selected_text_range(&mut self) -> Option<Range<usize>> {
        let (start_char, end_char) = self.session.selection().range();
        let start_u16 = self.session.rope().char_to_utf16(start_char);
        let end_u16 = self.session.rope().char_to_utf16(end_char);
        Some(start_u16..end_u16)
    }

    fn marked_text_range(&self) -> Option<Range<usize>> {
        let (start_char, end_char) = self.session.ime().active_range?;
        let start_u16 = self.session.rope().char_to_utf16(start_char);
        let end_u16 = self.session.rope().char_to_utf16(end_char);
        Some(start_u16..end_u16)
    }

    fn unmark_text(&mut self) {
        if self.session.ime().is_composing() {
            let committed = self.session.ime().preedit_text.clone();
            let _ = self.session.ime_commit(&committed);
        }
    }

    fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, text: &str) {
        if let Some(r) = range_utf16 {
            let start_char = self.session.rope().utf16_to_char(r.start);
            let end_char = self.session.rope().utf16_to_char(r.end);
            self.session.set_selection(Selection::new(start_char, end_char));
        }
        let _ = self.session.ime_commit(text);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
    ) {
        if !self.session.ime().is_composing() {
            if let Some(r) = range_utf16 {
                let start_char = self.session.rope().utf16_to_char(r.start);
                let end_char = self.session.rope().utf16_to_char(r.end);
                self.session.set_selection(Selection::new(start_char, end_char));
            }
        }
        let cursor_chars = new_selected_range_utf16.map(|r| {
            let c_start = utf16_offset_to_char_in_str(new_text, r.start);
            let c_end = utf16_offset_to_char_in_str(new_text, r.end);
            (c_start, c_end)
        });
        self.session.ime_preedit(new_text, cursor_chars);
    }
}

fn utf16_offset_to_char_in_str(text: &str, utf16_target: usize) -> usize {
    let mut u16_pos = 0;
    let mut char_pos = 0;
    for ch in text.chars() {
        if u16_pos >= utf16_target {
            break;
        }
        u16_pos += ch.len_utf16();
        char_pos += 1;
    }
    char_pos
}
