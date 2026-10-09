//! GPUI Component Input receives only a small Rope viewport, never the full note.
//! Edits are sent as compact UTF-16 ChangeSet JSON through real DocumentHub/Yrs.
use aquilum_native_common::{lab_file_paths, lab_profile_dir, NativeDocument, NativeHost};
use aquilum_native_rope_viewport::{TextModel, Viewport};
use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Entity,
    Subscription, Window, WindowBounds, WindowOptions,
};
use gpui_component::{input::{Input, InputEvent, InputState}, Root};
use std::io::Write as _;

const PAGE_CHARS: usize = 4096;
const MAX_EDIT_CHARS: usize = 8192;

struct Tab {
    document: NativeDocument,
    model: TextModel,
    viewport: Viewport,
    editor: Entity<InputState>,
    dirty: bool,
}
struct NativeEditor {
    _host: NativeHost,
    tabs: Vec<Tab>,
    selected: usize,
    status: String,
    _listeners: Vec<Subscription>,
}

impl NativeEditor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let paths = lab_file_paths().expect("Pass --file PATH (disposable Markdown copy)");
        let host = NativeHost::new(&lab_profile_dir()).expect("Aquilum core failed to start");
        let mut tabs = Vec::new();
        let mut listeners = Vec::new();
        for path in paths {
            let mut document = host.open(&path).expect("Failed to open Markdown fixture");
            let initial = document.take_initial_text();
            let model = TextModel::from_str(&initial);
            let viewport = model.window_at_char(0, PAGE_CHARS).expect("Invalid viewport");
            let initial_window = viewport.text.clone();
            let index = tabs.len();
            let editor = cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line()
                    .default_value(initial_window)
            });
            let listener = cx.subscribe(&editor, move |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(tab) = this.tabs.get_mut(index) {
                        let current = tab.editor.read(cx).value();
                        let actual: &str = current.as_ref();
                        tab.dirty = actual != tab.viewport.text.as_str();
                    }
                    cx.notify();
                }
            });
            listeners.push(listener);
            tabs.push(Tab { document, model, viewport, editor, dirty: false });
        }
        let model_bytes: usize = tabs.iter().map(|t| t.model.len_bytes()).sum();
        let visible_bytes: usize = tabs.iter().map(|t| t.viewport.text.len()).sum();
        println!(
            "AQUILUM_GPUI_ROPE_READY tabs={} model_bytes={} visible_bytes={} page_chars={}",
            tabs.len(), model_bytes, visible_bytes, PAGE_CHARS
        );
        let _ = std::io::stdout().flush();
        Self {
            _host: host,
            tabs,
            selected: 0,
            status: "Bounded GPUI Rope. Save commits one compact Yrs delta.".to_owned(),
            _listeners: listeners,
        }
    }

    fn show_window(tab: &mut Tab, start: usize, window: &mut Window, cx: &mut Context<Self>) {
        let viewport = tab.model.window_at_char(
            start.min(tab.model.len_chars()), PAGE_CHARS
        ).expect("Window index must be within Rope");
        let current_window = viewport.text.clone();
        tab.viewport = viewport;
        tab.editor.update(cx, |editor, cx| {
            editor.set_value(current_window, window, cx);
        });
        tab.dirty = false;
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.selected];
        let current = tab.editor.read(cx).value();
        let actual: &str = current.as_ref();
        // DocumentHub loads canonical LF. Normalize Windows paste before diff.
        let canonical = actual.replace("\r\n", "\n").replace('\r', "\n");
        let plan = match tab.model.propose_viewport_snapshot_change(
            &tab.viewport, &canonical, MAX_EDIT_CHARS
        ) {
            Ok(edit) => edit,
            Err(error) => {
                self.status = format!("Edit NOT sent; input retained: {error}");
                return;
            }
        };
        let start = tab.viewport.start_char;
        let Some(edit) = plan else {
            Self::show_window(tab, start, window, cx);
            self.status = "No changes to save".into();
            return;
        };
        match tab.document.push_json_change(edit.to_changeset_json()) {
            Ok(version) => {
                if let Err(error) = tab.model.commit_proposed_edit(edit) {
                    tab.dirty = true;
                    self.status = format!(
                        "CRITICAL: remote accepted but local Rope desynced ({error}); restart lab"
                    );
                    return;
                }
                Self::show_window(tab, start, window, cx);
                self.status = format!("Compact edit accepted by DocumentHub, version {version}");
            }
            Err(error) => {
                tab.dirty = true;
                self.status = format!("Save rejected by DocumentHub: {error}; input retained");
            }
        }
    }

    fn page(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.selected];
        let current = tab.editor.read(cx).value();
        let actual: &str = current.as_ref();
        if tab.dirty || actual != tab.viewport.text.as_str() {
            self.status = "Unsaved window; Save before changing pages".into();
            return;
        }
        let start = if forward {
            if tab.viewport.end_char == tab.model.len_chars() {
                tab.viewport.start_char
            } else {
                tab.viewport.end_char
            }
        } else {
            tab.viewport.start_char.saturating_sub(PAGE_CHARS)
        };
        Self::show_window(tab, start, window, cx);
        self.status = format!("New viewport starts at character {start}");
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.selected];
        let current = tab.editor.read(cx).value();
        let actual: &str = current.as_ref();
        if tab.dirty || actual != tab.viewport.text.as_str() {
            self.status = "Unsaved text; Save before Reload".into();
            return;
        }
        match tab.document.refresh() {
            Ok(text) => {
                tab.model = TextModel::from_str(&text);
                Self::show_window(tab, 0, window, cx);
                self.status = "Reloaded from real DocumentHub".into();
            }
            Err(error) => self.status = format!("Reload rejected: {error}"),
        }
    }
}

impl Render for NativeEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tab_bar = div().flex().gap_2();
        for i in 0..self.tabs.len() {
            let tab = &self.tabs[i];
            let label = format!(
                "{}{}",
                tab.document.path().file_name().unwrap_or_default().to_string_lossy(),
                if tab.dirty { " *" } else { "" }
            );
            tab_bar = tab_bar.child(div().id(i).cursor_pointer().p_2()
                .bg(rgb(0x313c53))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = i;
                    cx.notify();
                }))
                .child(label));
        }
        let tab = &self.tabs[self.selected];
        let editor = tab.editor.clone();
        let position = format!(
            "Character window {}..{} / {} | {} visible UTF-8 bytes | max edited {} chars",
            tab.viewport.start_char, tab.viewport.end_char, tab.model.len_chars(),
            tab.viewport.text.len(), MAX_EDIT_CHARS
        );
        div().flex().flex_col().size_full().bg(rgb(0x151922))
            .text_color(rgb(0xe9ebf3)).p_3().gap_2()
            .child(div().text_xl().child("Aquilum Native / GPUI bounded Rope"))
            .child(tab_bar)
            .child(div().flex().gap_2()
                .child(div().id("previous").p_2().bg(rgb(0x354e70)).cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.page(false, window, cx);
                        cx.notify();
                    })).child("Previous"))
                .child(div().id("next").p_2().bg(rgb(0x354e70)).cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.page(true, window, cx);
                        cx.notify();
                    })).child("Next"))
                .child(div().id("save").p_2().bg(rgb(0x35543c)).cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.save(window, cx);
                        cx.notify();
                    })).child("Save delta"))
                .child(div().id("reload").p_2().bg(rgb(0x354e70)).cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.reload(window, cx);
                        cx.notify();
                    })).child("Reload if clean"))
            )
            .child(div().child(position))
            .child(div().child(self.status.clone()))
            .child(div().id("markdown-editor").flex_1().min_h(px(220.0))
                .child(Input::new(&editor).h_full()))
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        gpui_component::init(cx);
        let bounds = Bounds::centered(None, size(px(1100.0), px(750.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let editor = cx.new(|cx| NativeEditor::new(window, cx));
                cx.new(|cx| Root::new(editor.into(), window, cx))
            },
        ).expect("Cannot open bounded GPUI editor");
        cx.activate(true);
    });
}
