use aquilum_native_common::{lab_file_paths, lab_profile_dir, NativeDocument, NativeHost};
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Element, Length, Theme};

struct Tab {
    document: NativeDocument,
    editor: text_editor::Content,
    dirty: bool,
}
struct State {
    _host: NativeHost,
    tabs: Vec<Tab>,
    selected: usize,
    status: String,
}
#[derive(Clone, Debug)]
enum Message {
    Switch(usize),
    Edit(text_editor::Action),
    Save,
    Reload,
}
impl State {
    fn boot() -> Self {
        let files = lab_file_paths().expect("Usage: iced --file PATH [--file PATH]");
        let host = NativeHost::new(&lab_profile_dir()).expect("Aquilum core startup failed");
        let mut tabs = Vec::new();
        for file in files {
            let mut document = host.open(&file).expect("Failed to open Markdown file");
            let content = document.take_initial_text();
            tabs.push(Tab {
                document,
                editor: text_editor::Content::with_text(&content),
                dirty: false,
            });
        }
        Self {
            _host: host,
            tabs,
            selected: 0,
            status: "Iced native editor prototype: explicit Save only".to_owned(),
        }
    }
}
fn update(state: &mut State, message: Message) {
    match message {
        Message::Switch(index) if index < state.tabs.len() => {
            state.selected = index;
            state.status = format!("Tab {}", index + 1);
        }
        Message::Switch(_) => (),
        Message::Edit(action) => {
            let tab = &mut state.tabs[state.selected];
            tab.editor.perform(action);
            tab.dirty = true;
        }
        Message::Save => {
            let tab = &mut state.tabs[state.selected];
            let current = tab.editor.text();
            match tab.document.save(&current) {
                Ok(version) => {
                    tab.dirty = false;
                    state.status = format!("Saved version {version}");
                }
                Err(error) => state.status = format!("Save rejected: {error}; editor still holds text"),
            }
        }
        Message::Reload => {
            let tab = &mut state.tabs[state.selected];
            if tab.dirty {
                state.status = "Unsaved text: save before reload".to_owned();
                return;
            }
            match tab.document.refresh() {
                Ok(content) => {
                    tab.editor = text_editor::Content::with_text(&content);
                    state.status = "Refreshed from Aquilum DocumentHub".to_owned();
                }
                Err(error) => state.status = format!("Reload failed: {error}"),
            }
        }
    }
}
fn view(state: &State) -> Element<'_, Message> {
    let mut tabs = row![].spacing(5);
    for (index, tab) in state.tabs.iter().enumerate() {
        let name = tab.document.path().file_name().unwrap_or_default().to_string_lossy();
        let label = format!("{}{}", name, if tab.dirty { " *" } else { "" });
        tabs = tabs.push(button(text(label)).on_press(Message::Switch(index)));
    }
    let selected = &state.tabs[state.selected];
    let editor = text_editor(&selected.editor)
        .placeholder("Markdown note")
        .on_action(Message::Edit)
        .height(Length::Fill);
    let body = column![
        text("Aquilum Native / Iced (experimental)").size(20),
        tabs,
        row![
            button("Save to document hub").on_press(Message::Save),
            button("Reload if clean").on_press(Message::Reload),
            text(&state.status)
        ].spacing(10),
        container(editor).width(Length::Fill).height(Length::Fill)
    ].spacing(8).padding(12);
    container(body).width(Length::Fill).height(Length::Fill).into()
}
fn main() -> iced::Result {
    iced::application(State::boot, update, view)
        .title("Aquilum Native - Iced lab")
        .theme(Theme::Dark)
        .run()
}
