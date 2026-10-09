//! Experimental native Markdown editor: ONLY the visible 80 lines are loaded
//! into Iced Content. The full file stays in compact indexed Rust storage.
//! This replaces Iced's expensive full-document Content model.
//!
//! Safety scope: explicit Save, stale-version rejection, no autosave or
//! crash recovery; use copies of Markdown documents, never live vault data.

use aquilum_native_common::{
    lab_file_paths, lab_profile_dir, line_viewport::LineViewport,
    NativeDocument, NativeHost,
};
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Element, Length, Theme};

const PAGE_LINES: usize = 80;

struct Tab {
    document: NativeDocument,
    buffer: LineViewport,
    editor: text_editor::Content,
    first_line: usize,
    view_from: usize,
    view_to: usize,
    dirty: bool,
}
impl Tab {
    fn new(mut document: NativeDocument) -> Self {
        let buffer = LineViewport::new(document.take_initial_text());
        let mut tab = Self {
            document, buffer, editor: text_editor::Content::new(),
            first_line: 0, view_from: 0, view_to: 0, dirty: false,
        };
        tab.load_page();
        tab
    }
    fn load_page(&mut self) {
        self.view_from = self.buffer.byte_of_line(self.first_line).unwrap_or(self.buffer.bytes());
        self.view_to = self.buffer.byte_of_line(self.first_line.saturating_add(PAGE_LINES))
            .unwrap_or(self.buffer.bytes());
        let visible = self.buffer.visible_text(self.first_line, PAGE_LINES);
        self.editor = text_editor::Content::with_text(visible);
    }
    fn next_page(&mut self) {
        let proposed = self.first_line.saturating_add(PAGE_LINES);
        if proposed < self.buffer.line_count() {
            self.first_line = proposed;
            self.load_page();
        }
    }
    fn previous_page(&mut self) {
        self.first_line = self.first_line.saturating_sub(PAGE_LINES);
        self.load_page();
    }
    /// Only the current editor window is copied after an edit, never the full
    /// document. The shared line index adjusts following byte offsets.
    fn apply_edit(&mut self, action: text_editor::Action) -> Result<(), String> {
        self.editor.perform(action);
        let edited_view = self.editor.text();
        let before = &self.buffer.text()[self.view_from..self.view_to];
        if edited_view != before {
            self.buffer.replace_bytes(self.view_from..self.view_to, &edited_view)
                .map_err(|e| format!("Invalid UTF-8 edit boundary: {e:?}"))?;
            self.view_to = self.view_from + edited_view.len();
            self.dirty = true;
        }
        Ok(())
    }
    fn save(&mut self) -> Result<u64, String> {
        let version = self.document.save(self.buffer.text())
            .map_err(|e| e.to_string())?;
        self.dirty = false;
        Ok(version)
    }
    fn reload_if_clean(&mut self) -> Result<(), String> {
        if self.dirty { return Err("Unsaved changes: save before reload".into()); }
        let content = self.document.refresh().map_err(|e| e.to_string())?;
        self.buffer = LineViewport::new(content);
        self.first_line = 0;
        self.load_page();
        Ok(())
    }
}
struct State {
    _host: NativeHost,
    tabs: Vec<Tab>,
    selected: usize,
    status: String,
}
#[derive(Clone, Debug)]
enum Message {
    Select(usize),
    Edit(text_editor::Action),
    Next,
    Previous,
    Save,
    Reload,
}
impl State {
    fn boot() -> Self {
        let paths = lab_file_paths().expect("Usage: iced-viewport --file PATH [--file PATH]");
        let host = NativeHost::new(&lab_profile_dir()).expect("Aquilum core startup failed");
        let mut tabs = Vec::new();
        for path in paths {
            let document = host.open(&path).expect("Cannot open Markdown test file");
            tabs.push(Tab::new(document));
        }
        Self {
            _host: host, tabs, selected: 0,
            status: "Bounded Iced view: each tab keeps only 80 editable lines in UI".into(),
        }
    }
}
fn update(state: &mut State, message: Message) {
    match message {
        Message::Select(i) if i < state.tabs.len() => {
            state.selected = i;
            state.status = format!("Selected tab {}", i + 1);
        }
        Message::Select(_) => (),
        Message::Next => state.tabs[state.selected].next_page(),
        Message::Previous => state.tabs[state.selected].previous_page(),
        Message::Edit(action) => {
            if let Err(err) = state.tabs[state.selected].apply_edit(action) {
                state.status = format!("Edit rejected: {err}");
            }
        }
        Message::Save => {
            match state.tabs[state.selected].save() {
                Ok(version) => state.status = format!("Saved with Aquilum DocumentHub at version {version}"),
                Err(err) => state.status = format!("Save rejected, buffer retained: {err}"),
            }
        }
        Message::Reload => {
            match state.tabs[state.selected].reload_if_clean() {
                Ok(()) => state.status = "Reloaded from real DocumentHub".into(),
                Err(err) => state.status = format!("Reload rejected: {err}"),
            }
        }
    }
}
fn view(state: &State) -> Element<'_, Message> {
    let mut tabs = row![].spacing(5);
    for (index, tab) in state.tabs.iter().enumerate() {
        let label = format!("{}{}",
            tab.document.path().file_name().unwrap_or_default().to_string_lossy(),
            if tab.dirty { " *" } else { "" });
        tabs = tabs.push(button(text(label)).on_press(Message::Select(index)));
    }
    let tab = &state.tabs[state.selected];
    let end_line = (tab.first_line + PAGE_LINES).min(tab.buffer.line_count());
    let heading = format!(
        "Document: {} bytes, {} logical lines | Visible: {}-{} | Model capacity: {} bytes",
        tab.buffer.bytes(), tab.buffer.line_count(), tab.first_line + 1,
        end_line, tab.buffer.backing_capacity_bytes()
    );
    let editor = text_editor(&tab.editor)
        .on_action(Message::Edit)
        .height(Length::Fill);
    let content = column![
        text("Aquilum Native / Iced virtual-page experiment").size(20),
        tabs,
        text(heading),
        row![
            button("Previous 80 lines").on_press(Message::Previous),
            button("Next 80 lines").on_press(Message::Next),
            button("Save").on_press(Message::Save),
            button("Reload if clean").on_press(Message::Reload)
        ].spacing(8),
        text(&state.status),
        container(editor).width(Length::Fill).height(Length::Fill),
    ].padding(12).spacing(8);
    container(content).width(Length::Fill).height(Length::Fill).into()
}
fn main() -> iced::Result {
    iced::application(State::boot, update, view)
        .title("Aquilum Native / Iced bounded editor experiment")
        .theme(Theme::Dark)
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn iced_content_text_roundtrip_for_viewport_lines() {
        for sample in ["", "text", "text\n", "first\nsecond\n", "Русский 🌍\n", "no newline"] {
            let content: text_editor::Content = text_editor::Content::with_text(sample);
            assert_eq!(content.text(), sample, "Iced must not silently normalize the Markdown page");
        }
    }
    #[test]
    fn changing_visible_page_preserves_hidden_content() {
        let tempfile = tempfile::tempdir().unwrap();
        let path = tempfile.path().join("note.md");
        let original = format!("{}\nHIDDEN 🌍", "line\n".repeat(100));
        std::fs::write(&path, &original).unwrap();
        let host = NativeHost::new(&tempfile.path().join("profile")).unwrap();
        let document = host.open(&path).unwrap();
        let mut tab = Tab::new(document);
        assert_eq!(tab.buffer.text(), original);
        tab.next_page();
        assert!(tab.first_line >= 80);
        assert_eq!(tab.buffer.text(), original);
        tab.previous_page();
        assert_eq!(tab.buffer.text(), original);
    }
}
