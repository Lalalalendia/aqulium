//! Native GPUI multiline text-editing probe over actual aquilum-core.
//!
//! Unlike the initial paged GPUI viewer, this stage uses the GPUI Component
//! text editing engine, with keyboard input, native IME, selection and undo.
//! Explicit manual Save still goes through DocumentHub with version checking.
//! This is an isolated lab: test copies only, no autosave or crash recovery.
use aquilum_native_common::{lab_file_paths, lab_profile_dir, NativeDocument, NativeHost};
use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Entity,
    Subscription, Window, WindowBounds, WindowOptions,
};
use gpui_component::{input::{Input, InputEvent, InputState}, Root};

struct Tab {
    document: NativeDocument,
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
        let paths = lab_file_paths().expect("Usage: editor --file PATH [--file PATH]");
        let host = NativeHost::new(&lab_profile_dir()).expect("Cannot open Aquilum core");
        let mut tabs = Vec::new();
        let mut listeners = Vec::new();
        for path in paths {
            let mut document = host.open(&path).expect("Cannot load test Markdown document");
            let initial = document.take_initial_text();
            let index = tabs.len();
            let editor = cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .default_value(initial)
            });
            let subscription = cx.subscribe(&editor, move |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(tab) = this.tabs.get_mut(index) {
                        tab.dirty = true;
                    }
                    cx.notify();
                }
            });
            listeners.push(subscription);
            tabs.push(Tab { document, editor, dirty: false });
        }
        Self {
            _host: host,
            tabs,
            selected: 0,
            status: "GPUI Component multiline editing; explicit Save only".to_owned(),
            _listeners: listeners,
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.selected];
        let source = tab.editor.read(cx).value();
        match tab.document.save(source.as_ref()) {
            Ok(version) => {
                tab.dirty = false;
                self.status = format!("Saved with DocumentHub, version {version}");
            }
            Err(error) => self.status = format!("Save rejected: {error}; buffer kept"),
        }
    }

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tab = &mut self.tabs[self.selected];
        if tab.dirty {
            self.status = "Unsaved changes; save before reload".into();
            return;
        }
        match tab.document.refresh() {
            Ok(source) => {
                tab.editor.update(cx, |editor, cx| editor.set_value(source, window, cx));
                tab.dirty = false;
                self.status = "Reloaded from real DocumentHub".to_owned();
            }
            Err(error) => self.status = format!("Reload failed: {error}"),
        }
    }
}

impl Render for NativeEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tab_bar = div().flex().gap_2();
        for i in 0..self.tabs.len() {
            let label = format!(
                "{}{}",
                self.tabs[i].document.path().file_name()
                    .unwrap_or_default().to_string_lossy(),
                if self.tabs[i].dirty { " *" } else { "" }
            );
            tab_bar = tab_bar.child(div().id(i).cursor_pointer().p_2()
                .bg(rgb(0x313c53))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = i;
                    cx.notify();
                }))
                .child(label));
        }
        let text_input = self.tabs[self.selected].editor.clone();
        div().flex().flex_col().size_full().bg(rgb(0x151922))
            .text_color(rgb(0xe9ebf3)).p_3().gap_2()
            .child(div().text_xl().child("Aquilum Native / GPUI editor experiment"))
            .child(tab_bar)
            .child(div().flex().gap_2()
                .child(div().id("save-document").p_2().bg(rgb(0x35543c)).cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save(cx);
                        cx.notify();
                    }))
                    .child("Save"))
                .child(div().id("reload-document").p_2().bg(rgb(0x354e70)).cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.reload(window, cx);
                        cx.notify();
                    }))
                    .child("Reload if clean"))
                .child(div().child(&self.status))
            )
            .child(div().id("markdown-editor").flex_1().min_h(px(220.0))
                .child(Input::new(&text_input).h_full()))
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
                cx.new(|cx| Root::new(editor, window, cx))
            },
        ).expect("Cannot open native GPUI editor window");
        cx.activate(true);
    });
}
