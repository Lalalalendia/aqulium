use crate::editor_view::{GpuiEditorAction, GpuiFrameState, GpuiImeBridge, GpuiRopeEditorView};
use aquilum_core::app_core::Core;
use aquilum_core::files::models::FileItem;
use aquilum_core::history::service::HistoryPage;
use aquilum_core::native_editor::{ColumnAlignment, EvaluatedDataviewBlock, KeyCommand};
use aquilum_core::native_shell::{
    BacklinksSnapshot, DecodedGraphEdge, DecodedGraphNode, GraphColorMode, NativeTab,
    NativeWorkspaceShell,
};
use aquilum_core::search::models::{NoteSuggestion, SearchResponse};
use aquilum_core::settings::models::AppConfig;
use aquilum_core::ui_state::models::TabKind;
use std::ops::Range;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuiSidebarMode {
    Files,
    Search,
    Templates,
    Hidden,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuiInspectorMode {
    Backlinks,
    History,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GpuiModalOverlay {
    None,
    QuickSwitcher { query: String },
    GlobalSearch { query: String },
    Settings,
}

#[derive(Debug)]
pub enum GpuiActiveCenterSurface {
    Editor {
        frame: GpuiFrameState,
        in_table: bool,
        dataview_blocks: Vec<EvaluatedDataviewBlock>,
    },
    Graph {
        nodes: Vec<(DecodedGraphNode, (f32, f32))>,
        edges: Vec<DecodedGraphEdge>,
        color_mode: GraphColorMode,
        focused_node: Option<u32>,
    },
    Reader {
        book_title: String,
        chapter_title: String,
        chapter_index: usize,
        paragraph_index: usize,
        progress_percent: i64,
        cfi: String,
        visible_paragraphs: Vec<String>,
    },
    EmptyState,
}

pub struct GpuiWindowLayoutFrame {
    pub window_title: String,
    pub theme: String,
    pub sidebar_mode: GpuiSidebarMode,
    pub inspector_mode: GpuiInspectorMode,
    pub modal: GpuiModalOverlay,
    pub root_files: Vec<FileItem>,
    pub tabs: Vec<NativeTab>,
    pub active_tab_id: Option<Uuid>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub center: GpuiActiveCenterSurface,
    pub backlinks: Option<BacklinksSnapshot>,
    pub history_page: Option<HistoryPage>,
    pub search_response: Option<SearchResponse>,
    pub quick_suggestions: Vec<NoteSuggestion>,
}

#[derive(Clone, Debug)]
pub enum GpuiWindowAction {
    OpenNote { relative_path: String, in_new_tab: bool },
    CreateNote { relative_path: String, content: String },
    CreateNoteFromTemplate { template_name: String, target_rel_path: String, date_iso: String },
    RenameNote { from_rel: String, to_rel: String },
    TrashNote { relative_path: String },
    CloseTab { tab_id: Uuid },
    NavigateBack,
    NavigateForward,
    FollowLinkAtCursor { in_new_tab: bool },
    Editor(GpuiEditorAction),
    TableInsertRowBelow,
    TableDeleteRow,
    TableInsertColumnAfter,
    TableDeleteColumn,
    TableAlignColumn(ColumnAlignment),
    TableNavigateCell { forward: bool },
    ToggleTaskAtLine { line: usize },
    OpenGraphTab,
    SetGraphColorMode(GraphColorMode),
    ClickGraphNode { screen_x: f32, screen_y: f32 },
    OpenBook { relative_path: String, cfi: Option<String> },
    ReaderNavigate { chapter: usize, paragraph: usize, now_ms: i64 },
    InsertBookQuoteIntoNote { selected_text: String, target_note_rel: String },
    SetSidebarMode(GpuiSidebarMode),
    SetInspectorMode(GpuiInspectorMode),
    OpenQuickSwitcher(String),
    OpenGlobalSearch(String),
    OpenSettings,
    CloseModal,
    SetTheme(String),
    RevertActiveNoteToVersion { version_id: Option<String> },
    SaveSession { now_ms: i64 },
}

/// Top-level native GPUI window controller hosting the entire Aquilum workspace UI
/// (Sidebar, TabBar, Editor with bounded 8192-char Rope viewport, Interactive Tables
/// & Dataview, 2D Force Graph, FB2/EPUB Reader, Backlinks/History Inspector, and Settings).
pub struct GpuiWorkspaceWindow {
    shell: NativeWorkspaceShell,
    sidebar_mode: GpuiSidebarMode,
    inspector_mode: GpuiInspectorMode,
    modal: GpuiModalOverlay,
    active_book_rel: Option<String>,
    viewport_size_px: (f32, f32),
}

impl GpuiWorkspaceWindow {
    pub fn open(
        core: Arc<Core>,
        workspace_root: &Path,
        window_id: impl Into<String>,
        now_ms: i64,
    ) -> Result<Self, String> {
        let shell = NativeWorkspaceShell::open(core, workspace_root, window_id, now_ms)?;
        Ok(Self {
            shell,
            sidebar_mode: GpuiSidebarMode::Files,
            inspector_mode: GpuiInspectorMode::Backlinks,
            modal: GpuiModalOverlay::None,
            active_book_rel: None,
            viewport_size_px: (1280.0, 800.0),
        })
    }

    pub fn shell(&self) -> &NativeWorkspaceShell {
        &self.shell
    }

    pub fn shell_mut(&mut self) -> &mut NativeWorkspaceShell {
        &mut self.shell
    }

    pub fn set_viewport_size_px(&mut self, width: f32, height: f32) {
        self.viewport_size_px = (width.max(320.0), height.max(240.0));
    }

    pub fn dispatch(&mut self, action: GpuiWindowAction) -> Result<(), String> {
        match action {
            GpuiWindowAction::OpenNote { relative_path, in_new_tab } => {
                self.active_book_rel = None;
                self.shell.open_note(&relative_path, in_new_tab)?;
            }
            GpuiWindowAction::CreateNote { relative_path, content } => {
                self.shell
                    .explorer
                    .create_note(&relative_path, &content)
                    .map_err(|e| format!("{e:?}"))?;
                self.active_book_rel = None;
                self.shell.open_note(&relative_path, false)?;
            }
            GpuiWindowAction::CreateNoteFromTemplate {
                template_name,
                target_rel_path,
                date_iso,
            } => {
                self.shell.templates.create_note_from_template(
                    self.shell.workspace_root(),
                    &template_name,
                    &target_rel_path,
                    &date_iso,
                )?;
                self.active_book_rel = None;
                self.shell.open_note(&target_rel_path, false)?;
            }
            GpuiWindowAction::RenameNote { from_rel, to_rel } => {
                self.shell
                    .explorer
                    .rename_note(&from_rel, &to_rel)
                    .map_err(|e| format!("{e:?}"))?;
            }
            GpuiWindowAction::TrashNote { relative_path } => {
                self.shell
                    .explorer
                    .trash_note(&relative_path)
                    .map_err(|e| format!("{e:?}"))?;
            }
            GpuiWindowAction::CloseTab { tab_id } => {
                self.shell.tabs.close_tab(tab_id);
            }
            GpuiWindowAction::NavigateBack => {
                if let Some(prev) = self.shell.tabs.nav_history.go_back() {
                    self.active_book_rel = None;
                    self.shell.open_note(&prev, false)?;
                }
            }
            GpuiWindowAction::NavigateForward => {
                if let Some(next) = self.shell.tabs.nav_history.go_forward() {
                    self.active_book_rel = None;
                    self.shell.open_note(&next, false)?;
                }
            }
            GpuiWindowAction::FollowLinkAtCursor { in_new_tab } => {
                if let Some(target) = self.shell.follow_link_at_cursor(in_new_tab)? {
                    if let aquilum_core::native_editor::InteractiveLinkTarget::ReaderQuote {
                        book_path,
                        ..
                    } = target
                    {
                        self.active_book_rel = Some(book_path);
                    } else {
                        self.active_book_rel = None;
                    }
                }
            }
            GpuiWindowAction::Editor(ed_action) => {
                let Some(editor) = self.shell.active_editor_mut() else {
                    return Ok(());
                };
                match ed_action {
                    GpuiEditorAction::Key(cmd) => editor.handle_key(cmd).map_err(|e| e.to_string())?,
                    GpuiEditorAction::MouseDown { viewport_char_offset, shift } => {
                        let vp = editor.viewport();
                        let doc_char = (vp.start_char + viewport_char_offset).min(editor.rope().len_chars());
                        let anchor = if shift { editor.selection().anchor } else { doc_char };
                        editor.set_selection(aquilum_core::native_editor::Selection::new(anchor, doc_char));
                    }
                    GpuiEditorAction::MouseDrag { viewport_char_offset } => {
                        let vp = editor.viewport();
                        let doc_char = (vp.start_char + viewport_char_offset).min(editor.rope().len_chars());
                        let anchor = editor.selection().anchor;
                        editor.set_selection(aquilum_core::native_editor::Selection::new(anchor, doc_char));
                    }
                    GpuiEditorAction::ScrollLines { delta_lines } => {
                        let vp = editor.viewport();
                        let target_line = if delta_lines < 0 {
                            vp.start_line.saturating_sub((-delta_lines) as usize)
                        } else {
                            (vp.start_line + delta_lines as usize)
                                .min(editor.rope().len_lines().saturating_sub(1))
                        };
                        editor.scroll_to_line(target_line);
                    }
                    GpuiEditorAction::SetViewportBudget { max_chars } => {
                        editor.set_viewport_budget_chars(max_chars);
                    }
                }
            }
            GpuiWindowAction::TableInsertRowBelow => {
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.table_insert_row_below().map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::TableDeleteRow => {
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.table_delete_current_row().map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::TableInsertColumnAfter => {
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.table_insert_column_after().map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::TableDeleteColumn => {
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.table_delete_current_column().map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::TableAlignColumn(align) => {
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.table_set_column_alignment(align).map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::TableNavigateCell { forward } => {
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.table_navigate_cell(forward).map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::ToggleTaskAtLine { line } => {
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.toggle_task_at_line(line).map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::OpenGraphTab => {
                self.active_book_rel = None;
                self.shell.tabs.open_graph_tab();
                let ws = self.shell.workspace_root().to_path_buf();
                let _ = self.shell.graph.refresh(&ws);
            }
            GpuiWindowAction::SetGraphColorMode(mode) => {
                self.shell.graph.set_color_mode(mode);
            }
            GpuiWindowAction::ClickGraphNode { screen_x, screen_y } => {
                let (w, h) = self.viewport_size_px;
                if let Some(node_idx) = self.shell.graph.hit_test_node(screen_x, screen_y, w, h, 16.0) {
                    self.shell.graph.focused_node = Some(node_idx);
                    if let Some(node) = self.shell.graph.nodes().get(node_idx as usize).cloned() {
                        let rel = Path::new(&node.path)
                            .strip_prefix(self.shell.workspace_root())
                            .unwrap_or_else(|_| Path::new(&node.path))
                            .to_string_lossy()
                            .replace('\\', "/");
                        self.shell.open_note(&rel, true)?;
                    }
                }
            }
            GpuiWindowAction::OpenBook { relative_path, cfi } => {
                self.shell.open_book(&relative_path, cfi.as_deref())?;
                self.active_book_rel = Some(relative_path.replace('\\', "/"));
            }
            GpuiWindowAction::ReaderNavigate { chapter, paragraph, now_ms } => {
                if let Some(book_rel) = self.active_book_rel.clone() {
                    if let Ok(reader) = self.shell.open_book(&book_rel, None) {
                        reader.navigate_to(chapter, paragraph, now_ms)?;
                    }
                }
            }
            GpuiWindowAction::InsertBookQuoteIntoNote { selected_text, target_note_rel } => {
                let Some(book_rel) = self.active_book_rel.clone() else {
                    return Err("no active book open".to_owned());
                };
                let quote_md = {
                    let reader = self.shell.open_book(&book_rel, None)?;
                    reader.format_linked_quote(&selected_text)
                };
                self.active_book_rel = None;
                self.shell.open_note(&target_note_rel, false)?;
                if let Some(ed) = self.shell.active_editor_mut() {
                    ed.handle_key(KeyCommand::MoveDocEnd { select: false })
                        .map_err(|e| e.to_string())?;
                    ed.handle_key(KeyCommand::InsertText(format!("\n{quote_md}")))
                        .map_err(|e| e.to_string())?;
                    ed.save_now().map_err(|e| e.to_string())?;
                }
            }
            GpuiWindowAction::SetSidebarMode(mode) => self.sidebar_mode = mode,
            GpuiWindowAction::SetInspectorMode(mode) => self.inspector_mode = mode,
            GpuiWindowAction::OpenQuickSwitcher(q) => {
                self.modal = GpuiModalOverlay::QuickSwitcher { query: q };
            }
            GpuiWindowAction::OpenGlobalSearch(q) => {
                self.modal = GpuiModalOverlay::GlobalSearch { query: q };
            }
            GpuiWindowAction::OpenSettings => {
                self.modal = GpuiModalOverlay::Settings;
            }
            GpuiWindowAction::CloseModal => {
                self.modal = GpuiModalOverlay::None;
            }
            GpuiWindowAction::SetTheme(theme) => {
                self.shell.settings.set_theme(&theme)?;
            }
            GpuiWindowAction::RevertActiveNoteToVersion { version_id } => {
                if let Some(tab) = self.shell.tabs.active_tab().cloned() {
                    if let Some(rel) = tab.relative_path {
                        let abs = self.shell.workspace_root().join(&rel);
                        self.shell
                            .history
                            .revert_to_version(&abs, version_id.as_deref())?;
                        if let Some(ed) = self.shell.active_editor_mut() {
                            ed.sync_external().map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            GpuiWindowAction::SaveSession { now_ms } => {
                self.shell.save_session(now_ms)?;
            }
        }
        Ok(())
    }

    /// Renders a complete, self-contained layout snapshot for GPUI's `Render::render` pass.
    pub fn render_window_frame(&self) -> GpuiWindowLayoutFrame {
        let cfg: AppConfig = self.shell.settings.config();
        let root_files = self.shell.explorer.list_root().unwrap_or_default();
        let tabs = self.shell.tabs.tabs().to_vec();
        let active_tab_id = self.shell.tabs.active_tab_id();

        let center = if let Some(book_rel) = &self.active_book_rel {
            if let Some(reader) = self.shell.reader(book_rel) {
                let (c_idx, p_idx) = reader.current_position();
                let chapter = &reader.book().chapters[c_idx];
                let visible_paragraphs = chapter
                    .paragraphs
                    .iter()
                    .skip(p_idx)
                    .take(12)
                    .map(|p| p.text.clone())
                    .collect();
                GpuiActiveCenterSurface::Reader {
                    book_title: reader.book().metadata.title.clone(),
                    chapter_title: chapter.title.clone(),
                    chapter_index: c_idx,
                    paragraph_index: p_idx,
                    progress_percent: reader.progress_percent(),
                    cfi: reader.current_cfi(),
                    visible_paragraphs,
                }
            } else {
                GpuiActiveCenterSurface::EmptyState
            }
        } else if let Some(active_tab) = self.shell.tabs.active_tab() {
            match active_tab.kind {
                TabKind::Graph => {
                    let (w, h) = self.viewport_size_px;
                    let nodes = self
                        .shell
                        .graph
                        .nodes()
                        .iter()
                        .map(|n| (n.clone(), self.shell.graph.world_to_screen(n.x, n.y, w, h)))
                        .collect();
                    GpuiActiveCenterSurface::Graph {
                        nodes,
                        edges: self.shell.graph.edges().to_vec(),
                        color_mode: self.shell.graph.color_mode,
                        focused_node: self.shell.graph.focused_node,
                    }
                }
                TabKind::Document => {
                    if let Some(ed) = self.shell.active_editor() {
                        let vp = ed.viewport();
                        let metrics = ed.metrics();
                        let lc = ed.rope().char_to_line_col(ed.selection().head);
                        let in_table = aquilum_core::native_editor::parse_table_at_line(
                            ed.rope(),
                            lc.line,
                        )
                        .is_some();
                        let dataview_blocks =
                            ed.evaluate_dataview_blocks(self.shell.workspace_root(), 0);
                        GpuiActiveCenterSurface::Editor {
                            frame: GpuiFrameState {
                                title: active_tab.title.clone(),
                                dirty: metrics.dirty,
                                version: metrics.version,
                                cursor_line: lc.line + 1,
                                cursor_column: lc.column_chars + 1,
                                viewport: vp,
                                metrics,
                            },
                            in_table,
                            dataview_blocks,
                        }
                    } else {
                        GpuiActiveCenterSurface::EmptyState
                    }
                }
                TabKind::Empty => GpuiActiveCenterSurface::EmptyState,
            }
        } else {
            GpuiActiveCenterSurface::EmptyState
        };

        let (backlinks, history_page) = if let Some(ed) = self.shell.active_editor() {
            let bl = (self.inspector_mode == GpuiInspectorMode::Backlinks)
                .then(|| self.shell.backlinks.inspect(ed.path(), Some(20)).ok())
                .flatten();
            let hp = (self.inspector_mode == GpuiInspectorMode::History)
                .then(|| self.shell.history.page(ed.path(), 0, 50));
            (bl, hp)
        } else {
            (None, None)
        };

        let (search_response, quick_suggestions) = match &self.modal {
            GpuiModalOverlay::GlobalSearch { query } => {
                (self.shell.search.search(query, Some(30)).ok(), Vec::new())
            }
            GpuiModalOverlay::QuickSwitcher { query } => {
                (None, self.shell.search.suggest(query, 12).unwrap_or_default())
            }
            _ => (None, Vec::new()),
        };

        let window_title = self
            .shell
            .tabs
            .active_tab()
            .map(|t| format!("{} — Aquilum", t.title))
            .unwrap_or_else(|| "Aquilum".to_owned());

        GpuiWindowLayoutFrame {
            window_title,
            theme: cfg.ui.theme,
            sidebar_mode: self.sidebar_mode,
            inspector_mode: self.inspector_mode,
            modal: self.modal.clone(),
            root_files,
            tabs,
            active_tab_id,
            can_go_back: self.shell.tabs.nav_history.can_go_back(),
            can_go_forward: self.shell.tabs.nav_history.can_go_forward(),
            center,
            backlinks,
            history_page,
            search_response,
            quick_suggestions,
        }
    }

pub fn close(self, now_ms: i64) {
        self.shell.shutdown(now_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aquilum_core::app_core::{Core, CoreEvent, EventSink};
    use std::fs;
    use std::sync::Mutex;

    #[derive(Default)]
    struct WinSink(Mutex<Vec<CoreEvent>>);

    impl EventSink for WinSink {
        fn emit(&self, event: CoreEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    #[test]
    fn gpui_workspace_window_end_to_end_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let sink = Arc::new(WinSink::default());
        let data_dir = dir.path().join("data");
        fs::create_dir_all(&data_dir).unwrap();
        let core = Core::open(&data_dir, Arc::clone(&sink) as Arc<dyn EventSink>);
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        let mut win = GpuiWorkspaceWindow::open(Arc::clone(&core), &vault, "gpui-test-win", 1_000).unwrap();
        win.dispatch(GpuiWindowAction::CreateNote {
            relative_path: "Первая.md".to_owned(),
            content: "# Первая\n| Кол 1 | Кол 2 |\n| --- | --- |\n| А | Б |\n- [ ] Задача 1\n".to_owned(),
        })
        .unwrap();

        let frame1 = win.render_window_frame();
        assert_eq!(frame1.tabs.len(), 1);
        assert!(matches!(frame1.center, GpuiActiveCenterSurface::Editor { .. }));

        // Toggle task and insert table row through window actions
        win.dispatch(GpuiWindowAction::ToggleTaskAtLine { line: 4 }).unwrap();
        win.dispatch(GpuiWindowAction::OpenGlobalSearch("Первая".to_owned())).unwrap();
        let frame2 = win.render_window_frame();
        assert!(matches!(frame2.modal, GpuiModalOverlay::GlobalSearch { .. }));

        // Switch to Graph tab and back
        win.dispatch(GpuiWindowAction::OpenGraphTab).unwrap();
        let frame3 = win.render_window_frame();
        assert!(matches!(frame3.center, GpuiActiveCenterSurface::Graph { .. }));

        win.dispatch(GpuiWindowAction::SaveSession { now_ms: 2_000 }).unwrap();
        win.close(3_000);
    }
}
