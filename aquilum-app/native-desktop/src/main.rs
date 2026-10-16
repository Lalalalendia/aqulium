//! Native Aquilum OS window prototype.
//!
//! This executable deliberately uses an independent Cargo workspace. It
//! consumes the existing Rust scene graph and controller without changing
//! the production Tauri runtime or claiming that GUI cutover is ready.

use aquilum_core::app_core::{Core, CoreEvent, EventSink};
use aquilum_core::native_editor::KeyCommand;
use aquilum_native_gpui::{
    build_window_scene, GpuiEditorAction, GpuiSceneGraph, GpuiWindowAction,
    GpuiWorkspaceWindow, HitboxTarget, SceneTextRun,
};
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct NativeSink;

impl EventSink for NativeSink {
    fn emit(&self, _event: CoreEvent) {
        // A future runtime change must wire external file events into the
        // controller. The initial window does not pretend to process them.
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn parse_args() -> Result<(PathBuf, PathBuf, bool), String> {
    let mut args = std::env::args_os().skip(1);
    let Some(target) = args.next() else {
        return Err(
            "Usage: aquilum-native-desktop <vault-dir-or-note.md> [--app-data <directory>]"
                .to_owned(),
        );
    };
    let target = PathBuf::from(target);
    let mut app_data = std::env::temp_dir().join("aquilum-native-desktop-data");
    let mut smoke_launch = false;
    while let Some(arg) = args.next() {
        if arg == "--smoke-launch" {
            smoke_launch = true;
        } else if arg == "--app-data" {
            let value = args
                .next()
                .ok_or_else(|| "--app-data requires a directory".to_owned())?;
            app_data = PathBuf::from(value);
        } else {
            return Err(format!("unknown option: {}", arg.to_string_lossy()));
        }
    }
    if !target.exists() {
        return Err(format!("workspace or note does not exist: {}", target.display()));
    }
    Ok((target, app_data, smoke_launch))
}

fn launch(target: &Path, app_data: &Path, smoke_launch: bool) -> Result<(), String> {
    let (root, initial_note) = if target.is_dir() {
        (target.to_path_buf(), None)
    } else {
        let parent = target
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let name = target
            .file_name()
            .ok_or_else(|| "note has no filename".to_owned())?
            .to_string_lossy()
            .into_owned();
        (parent.to_path_buf(), Some(name))
    };

    std::fs::create_dir_all(app_data)
        .map_err(|e| format!("failed to create application data directory: {e}"))?;
    let core = Core::open(app_data, Arc::new(NativeSink));
    let mut window = GpuiWorkspaceWindow::open(Arc::clone(&core), &root, "egui-window", now_ms())?;

    if let Some(note) = initial_note {
        window.dispatch(GpuiWindowAction::OpenNote {
            relative_path: note,
            in_new_tab: false,
        })?;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Aquilum Native (experimental)")
            .with_inner_size([1240.0, 800.0]),
        ..Default::default()
    };
    let app = NativeApp {
        window: Some(window),
        scene: GpuiSceneGraph::default(),
        dragging_selection: false,
        error: None,
        smoke_launch,
        frames_painted: 0,
        smoke_reported: false,
    };
    eframe::run_native(
        "Aquilum Native (experimental)",
        options,
        Box::new(move |_cc| Ok(Box::new(app))),
    )
    .map_err(|e| format!("unable to open native window: {e}"))
}

/// Convert physical key events to the same editor commands tested by the
/// headless Rust model. Text insertion is handled by Event::Text or OS IME.
fn key_command(key: egui::Key, modifiers: egui::Modifiers) -> Option<KeyCommand> {
    use egui::Key;
    let primary = modifiers.command || modifiers.ctrl;
    let select = modifiers.shift;
    if primary {
        return match key {
            Key::S => Some(KeyCommand::Save),
            Key::A => Some(KeyCommand::SelectAll),
            Key::Z if select => Some(KeyCommand::Redo),
            Key::Z => Some(KeyCommand::Undo),
            Key::Y => Some(KeyCommand::Redo),
            Key::ArrowLeft => Some(KeyCommand::MoveWordLeft { select }),
            Key::ArrowRight => Some(KeyCommand::MoveWordRight { select }),
            Key::Backspace => Some(KeyCommand::DeleteWordBackward),
            Key::Delete => Some(KeyCommand::DeleteWordForward),
            _ => None,
        };
    }
    match key {
        Key::ArrowLeft => Some(KeyCommand::MoveLeft { select }),
        Key::ArrowRight => Some(KeyCommand::MoveRight { select }),
        Key::ArrowUp => Some(KeyCommand::MoveUp { select }),
        Key::ArrowDown => Some(KeyCommand::MoveDown { select }),
        Key::Home => Some(KeyCommand::MoveLineStart { select }),
        Key::End => Some(KeyCommand::MoveLineEnd { select }),
        Key::PageUp => Some(KeyCommand::PageUp { select, lines: 24 }),
        Key::PageDown => Some(KeyCommand::PageDown { select, lines: 24 }),
        Key::Enter => Some(KeyCommand::InsertNewline),
        Key::Tab if select => Some(KeyCommand::OutdentTab),
        Key::Tab => Some(KeyCommand::InsertTab),
        Key::Backspace => Some(KeyCommand::Backspace),
        Key::Delete => Some(KeyCommand::Delete),
        _ => None,
    }
}

/// Approximate mouse hit-testing for plain text. The scene currently does
/// not expose shaped glyph bounding boxes, so this is NOT pixel-perfect.
/// Never use byte indices: Markdown editor offsets are Unicode scalar indices.
fn editor_char_offset(text: &str, runs: &[SceneTextRun], point: egui::Pos2) -> usize {
    let candidates: Vec<&SceneTextRun> = runs
        .iter()
        .filter(|run| !run.monospace && run.font_size_px >= 14.0 && run.font_size_px <= 17.0)
        .collect();
    if candidates.is_empty() {
        return 0;
    }
    let (line_index, run) = candidates
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            (a.y - point.y)
                .abs()
                .total_cmp(&(b.y - point.y).abs())
        })
        .map(|(i, run)| (i, *run))
        .unwrap();

    let advance = run.font_size_px * 0.55;
    let column = ((point.x - run.x).max(0.0) / advance.max(1.0)).round() as usize;
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if index == line_index {
            return (offset + column.min(line.trim_end_matches('\n').chars().count()))
                .min(text.chars().count());
        }
        offset += line.chars().count();
    }
    text.chars().count()
}

struct NativeApp {
    window: Option<GpuiWorkspaceWindow>,
    scene: GpuiSceneGraph,
    dragging_selection: bool,
    error: Option<String>,
    smoke_launch: bool,
    frames_painted: u64,
    smoke_reported: bool,
}

impl NativeApp {
    fn dispatch(&mut self, action: GpuiWindowAction) {
        if let Some(window) = &mut self.window {
            if let Err(error) = window.dispatch(action) {
                self.error = Some(error);
            }
        }
    }

    fn handle_pointer_down(&mut self, pos: egui::Pos2, modifiers: egui::Modifiers) {
        let Some(hitbox) = self.scene.hit_test(pos.x, pos.y).cloned() else {
            return;
        };
        match hitbox {
            HitboxTarget::EditorSurface => {
                self.dragging_selection = true;
                self.editor_pointer_move(pos, modifiers.shift, true);
            }
            HitboxTarget::TabSelect(tab_id) => {
                // NativeTabManager does not expose activate_tab(); reopening an
                // existing document with in_new_tab=true reuses and activates it.
                let relative_path = self.window.as_ref().and_then(|window| {
                    window.shell().tabs.tabs().iter()
                        .find(|tab| tab.tab_id == tab_id)
                        .and_then(|tab| tab.relative_path.clone())
                });
                if let Some(relative_path) = relative_path {
                    self.dispatch(GpuiWindowAction::OpenNote {
                        relative_path,
                        in_new_tab: true,
                    });
                } else {
                    self.dispatch(GpuiWindowAction::OpenGraphTab);
                }
            }
            HitboxTarget::TabClose(tab_id) => {
                self.dispatch(GpuiWindowAction::CloseTab { tab_id });
            }
            HitboxTarget::NavBack => self.dispatch(GpuiWindowAction::NavigateBack),
            HitboxTarget::NavForward => self.dispatch(GpuiWindowAction::NavigateForward),
            HitboxTarget::OpenGraphTab => self.dispatch(GpuiWindowAction::OpenGraphTab),
            HitboxTarget::SidebarItem(file_id) => {
                self.dispatch(GpuiWindowAction::ActivateSidebarItem {
                    file_id,
                    in_new_tab: false,
                });
            }
            HitboxTarget::GraphNode(_) => self.dispatch(GpuiWindowAction::ClickGraphNode {
                screen_x: pos.x,
                screen_y: pos.y,
            }),
            HitboxTarget::ModalDismiss => self.dispatch(GpuiWindowAction::CloseModal),
        }
    }

    fn editor_pointer_move(&mut self, pos: egui::Pos2, shift: bool, is_down: bool) {
        let Some(window) = &self.window else { return };
        let Some(editor) = window.shell().active_editor() else { return };
        let viewport = editor.viewport();
        let offset = editor_char_offset(&viewport.text, &self.scene.text_runs, pos);
        let event = if is_down {
            GpuiEditorAction::MouseDown { viewport_char_offset: offset, shift }
        } else {
            GpuiEditorAction::MouseDrag { viewport_char_offset: offset }
        };
        self.dispatch(GpuiWindowAction::Editor(event));
    }

    fn handle_ime(&mut self, event: egui::ImeEvent) {
        let Some(editor) = self.window.as_mut().and_then(|w| w.shell_mut().active_editor_mut())
        else { return };
        match event {
            egui::ImeEvent::Preedit(text) => {
                if text.is_empty() {
                    editor.ime_cancel();
                } else {
                    let cursor = text.chars().count();
                    editor.ime_preedit(&text, Some((cursor, cursor)));
                }
            }
            egui::ImeEvent::Commit(text) => {
                if let Err(error) = editor.ime_commit(&text) {
                    self.error = Some(error.to_string());
                }
            }
            egui::ImeEvent::Disabled => editor.ime_cancel(),
            egui::ImeEvent::Enabled => {}
        }
    }

    fn copy_selection(&mut self, ctx: &egui::Context, cut: bool) {
        let Some(window) = &mut self.window else { return };
        let Some(editor) = window.shell_mut().active_editor_mut() else { return };
        let (start, end) = editor.selection().range();
        if start == end { return }
        let text = editor.rope().slice_chars(start, end);
        ctx.output_mut(|output| output.copied_text = text);
        if cut {
            if let Err(error) = editor.handle_key(KeyCommand::Cut) {
                self.error = Some(error.to_string());
            }
        }
    }

    fn handle_event(&mut self, ctx: &egui::Context, event: egui::Event) {
        match event {
            egui::Event::Text(text) if !text.is_empty() => {
                self.dispatch(GpuiWindowAction::Editor(GpuiEditorAction::Key(
                    KeyCommand::InsertText(text),
                )));
            }
            egui::Event::Paste(text) if !text.is_empty() => {
                self.dispatch(GpuiWindowAction::Editor(GpuiEditorAction::Key(
                    KeyCommand::Paste(text),
                )));
            }
            egui::Event::Copy => self.copy_selection(ctx, false),
            egui::Event::Cut => self.copy_selection(ctx, true),
            egui::Event::Ime(event) => self.handle_ime(event),
            egui::Event::Key { key, modifiers, pressed: true, .. } => {
                if key == egui::Key::Escape {
                    self.dispatch(GpuiWindowAction::CloseModal);
                } else if let Some(command) = key_command(key, modifiers) {
                    self.dispatch(GpuiWindowAction::Editor(GpuiEditorAction::Key(command)));
                }
            }
            egui::Event::PointerButton {
                pos, button: egui::PointerButton::Primary,
                pressed, modifiers,
            } => {
                if pressed {
                    self.handle_pointer_down(pos, modifiers);
                } else {
                    self.dragging_selection = false;
                }
            }
            egui::Event::PointerMoved(pos) if self.dragging_selection => {
                self.editor_pointer_move(pos, false, false);
            }
            _ => {}
        }
    }

    fn paint_scene(&self, ctx: &egui::Context) {
        let painter = ctx.layer_painter(egui::LayerId::background());
        for quad in &self.scene.quads {
            let rect = egui::Rect::from_min_size(
                egui::pos2(quad.bounds.x, quad.bounds.y),
                egui::vec2(quad.bounds.width.max(0.0), quad.bounds.height.max(0.0)),
            );
            let fill = rgb(quad.bg_rgb);
            painter.rect_filled(rect, quad.corner_radius_px, fill);
            if let Some(border) = quad.border_rgb {
                painter.rect_stroke(
                    rect,
                    quad.corner_radius_px,
                    egui::Stroke::new(1.0_f32, rgb(border)),
                );
            }
        }
        for line in &self.scene.lines {
            painter.line_segment(
                [egui::pos2(line.x1, line.y1), egui::pos2(line.x2, line.y2)],
                egui::Stroke::new(line.stroke_width_px, rgb(line.color_rgb)),
            );
        }
        for text in &self.scene.text_runs {
            let family = if text.monospace {
                egui::FontFamily::Monospace
            } else {
                egui::FontFamily::Proportional
            };
            painter.text(
                egui::pos2(text.x, text.y),
                egui::Align2::LEFT_TOP,
                &text.text,
                egui::FontId::new(text.font_size_px, family),
                rgb(text.color_rgb),
            );
        }
        if let Some(error) = &self.error {
            let height = ctx.input(|input| input.screen_rect().height());
            painter.text(
                egui::pos2(18.0, height - 30.0),
                egui::Align2::LEFT_BOTTOM,
                error,
                egui::FontId::monospace(12.0),
                egui::Color32::RED,
            );
        }
    }
}

fn rgb((r, g, b): (u8, u8, u8)) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

impl eframe::App for NativeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Consume actual winit events from eframe, not synthesized GUI receipts.
        let events = ctx.input(|input| input.events.clone());
        for event in events {
            self.handle_event(ctx, event);
        }
        // eframe 0.29 exposes frame-local scroll delta outside raw Event
        // matching. Translate physical wheel/trackpad movement into bounded
        // viewport scrolling when a document editor is active.
        let wheel_y = ctx.input(|input| input.raw_scroll_delta.y);
        if wheel_y.abs() >= 1.0 {
            let lines = (wheel_y.abs() / 24.0).round().max(1.0) as isize;
            self.dispatch(GpuiWindowAction::Editor(GpuiEditorAction::ScrollLines {
                delta_lines: if wheel_y > 0.0 { -lines } else { lines },
            }));
        }
        if let Some(window) = &mut self.window {
            let rect = ctx.input(|input| input.screen_rect());
            let width = rect.width().max(320.0);
            let height = rect.height().max(240.0);
            window.set_viewport_size_px(width, height);
            let frame = window.render_window_frame();
            self.scene = build_window_scene(
                &frame,
                window.shell().theme_palette(),
                window.shell().ui_scale,
                width,
                height,
            );
        }
        self.paint_scene(ctx);
        self.frames_painted += 1;
        if self.smoke_launch && !self.smoke_reported && self.frames_painted >= 3 {
            // The marker is emitted only after eframe has invoked the actual
            // OS-backed render loop and the scene contains painted quads.
            if !self.scene.quads.is_empty() {
                println!(
                    "AQUILUM_NATIVE_OS_WINDOW_SMOKE frames={} quads={}",
                    self.frames_painted,
                    self.scene.quads.len()
                );
                self.smoke_reported = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(window) = self.window.take() {
            window.close(now_ms());
        }
    }
}

fn main() {
    if let Err(error) = parse_args()
        .and_then(|(target, app_data, smoke_launch)| launch(&target, &app_data, smoke_launch))
    {
        eprintln!("Aquilum native desktop: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_os_key_mapping_preserves_save_and_selection() {
        assert_eq!(
            key_command(egui::Key::S, egui::Modifiers { ctrl: true, ..Default::default() }),
            Some(KeyCommand::Save),
        );
        assert_eq!(
            key_command(egui::Key::Z, egui::Modifiers { command: true, shift: true, ..Default::default() }),
            Some(KeyCommand::Redo),
        );
        assert_eq!(
            key_command(egui::Key::ArrowLeft, egui::Modifiers { shift: true, ..Default::default() }),
            Some(KeyCommand::MoveLeft { select: true }),
        );
    }

    #[test]
    fn native_os_mouse_offset_counts_unicode_chars_not_utf8_bytes() {
        let runs = vec![
            SceneTextRun {
                x: 100.0, y: 60.0, text: "Русский".to_owned(),
                font_size_px: 15.0, color_rgb: (0, 0, 0), bold: false, monospace: false,
            },
            SceneTextRun {
                x: 100.0, y: 84.0, text: "Text".to_owned(),
                font_size_px: 15.0, color_rgb: (0, 0, 0), bold: false, monospace: false,
            },
        ];
        let text = "Русский\nText";
        assert_eq!(
            editor_char_offset(text, &runs, egui::pos2(100.0, 84.0)),
            8,
        );
        assert_eq!(
            editor_char_offset(text, &runs, egui::pos2(1000.0, 60.0)),
            7,
        );
    }
}
