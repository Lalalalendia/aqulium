use aquilum_native_common::{lab_file_paths, lab_profile_dir, NativeDocument, NativeHost};
use gpui::{div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Window, WindowBounds, WindowOptions};

struct Tab {
    document: NativeDocument,
    text: String,
    dirty: bool,
}
struct NativeGpui {
    _host: NativeHost,
    tabs: Vec<Tab>,
    selected: usize,
    page: usize,
    status: String,
}
impl NativeGpui {
    fn new() -> Self {
        let paths = lab_file_paths().expect("Usage: gpui --file PATH [--file PATH]");
        let host = NativeHost::new(&lab_profile_dir()).expect("Cannot initialize Aquilum core");
        let mut tabs = Vec::new();
        for path in paths {
            let mut document = host.open(&path).expect("Cannot open Markdown document");
            let text = document.take_initial_text();
            tabs.push(Tab { document, text, dirty: false });
        }
        Self {
            _host: host, tabs, selected: 0, page: 0,
            status: "GPUI viewer prototype; explicit Save only".to_owned(),
        }
    }
    fn save(&mut self) {
        let tab = &mut self.tabs[self.selected];
        match tab.document.save(&tab.text) {
            Ok(version) => {
                tab.dirty = false;
                self.status = format!("Saved at version {version}");
            }
            Err(error) => self.status = format!("Save rejected: {error}"),
        }
    }
}
impl Render for NativeGpui {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = &self.tabs[self.selected];
        let line_count = tab.text.lines().count();
        let last_page = line_count.saturating_sub(1) / 80;
        let excerpt = tab.text.lines().skip(self.page * 80).take(80)
            .collect::<Vec<_>>().join("\n");
        let heading = format!(
            "{} | {} bytes | {} lines | {}",
            tab.document.path().display(), tab.text.len(), line_count,
            if tab.dirty { "unsaved" } else { "clean" },
        );
        let mut tabs = div().flex().gap_2();
        for index in 0..self.tabs.len() {
            let title = self.tabs[index].document.path().file_name()
                .unwrap_or_default().to_string_lossy().to_string();
            tabs = tabs.child(div().id(format!("tab-{index}")).cursor_pointer().p_2().bg(rgb(0x354057))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.selected = index;
                    this.page = 0;
                    cx.notify();
                }))
                .child(title));
        }
        div().flex().flex_col().size_full().bg(rgb(0x151922))
            .text_color(rgb(0xe9eaf2)).p_3().gap_3()
            .child(div().text_xl().child("Aquilum Native / GPUI shell"))
            .child(tabs)
            .child(div().child(heading))
            .child(div().flex().gap_2()
                .child(div().id("prev-page").p_2().bg(rgb(0x385578)).cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.page = this.page.saturating_sub(1);
                        cx.notify();
                    }))
                    .child("Previous page"))
                .child(div().id("next-page").p_2().bg(rgb(0x385578)).cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = (this.page + 1).min(last_page);
                        cx.notify();
                    }))
                    .child("Next page"))
                .child(div().id("append-line").p_2().bg(rgb(0x42644d)).cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        let tab = &mut this.tabs[this.selected];
                        tab.text.push_str("\nGPUI native prototype appended a line.\n");
                        tab.dirty = true;
                        cx.notify();
                    }))
                    .child("Append test line"))
                .child(div().id("save-document").p_2().bg(rgb(0x42644d)).cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save();
                        cx.notify();
                    }))
                    .child("Save"))
            )
            .child(div().child(format!("Page {}/{} | {}", self.page + 1, last_page + 1, self.status)))
            .child(div().id("preview-scroll").flex_1().overflow_y_scroll().p_3().bg(rgb(0x222a36))
                .child(excerpt))
    }
}
fn main() {
    let initial = NativeGpui::new();
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1000.0), px(700.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| initial),
        ).expect("Cannot open GPUI window");
        cx.activate(true);
    });
}
