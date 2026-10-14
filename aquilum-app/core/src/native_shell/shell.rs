use super::graph_view::NativeGraphView;
use super::settings_view::{NativeSettingsController, NativeTemplateService};
use super::sidebar::{
    BacklinksInspector, GlobalSearchController, HistoryInspector, WorkspaceExplorer,
};
use super::shortcuts_theme_i18n::{Locale, ThemePalette, UiScaleState};
use super::tabs::NativeTabManager;
use super::trash_view::{NativeTrashController, WikixivInspector};
use crate::native_reader::BookLibraryController;
use crate::app_core::{Core, CoreEvent};
use crate::mcp::bridge::{Disposition, Navigation};
use crate::native_editor::{InteractiveLinkTarget, NativeEditorSession};
use crate::native_reader::NativeBookReader;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

/// Unified native Rust application shell hosting all Aquilum stages (Stages 1–6)
/// in-process on top of `Arc<Core>` without WebView2 or Tauri IPC overhead.
pub struct NativeWorkspaceShell {
    core: Arc<Core>,
    workspace_root: PathBuf,
    pub tabs: NativeTabManager,
    pub explorer: WorkspaceExplorer,
    pub search: GlobalSearchController,
    pub backlinks: BacklinksInspector,
    pub history: HistoryInspector,
    pub graph: NativeGraphView,
    pub settings: NativeSettingsController,
    pub templates: NativeTemplateService,
    pub trash: NativeTrashController,
    pub wikixiv: WikixivInspector,
    pub book_library: BookLibraryController,
    pub ui_scale: UiScaleState,
    editors: HashMap<String, NativeEditorSession>,
    editor_lru: VecDeque<String>,
    readers: HashMap<String, NativeBookReader>,
}

impl NativeWorkspaceShell {
    pub fn open(
        core: Arc<Core>,
        workspace_root: &Path,
        window_id: impl Into<String>,
        now_ms: i64,
    ) -> Result<Self, String> {
        let _ = core.search.prepare(&workspace_root.to_string_lossy());
        let tabs = NativeTabManager::open(Arc::clone(&core), workspace_root, window_id, now_ms)?;
        let explorer = WorkspaceExplorer::new(Arc::clone(&core), workspace_root);
        let search = GlobalSearchController::new(Arc::clone(&core), workspace_root);
        let backlinks = BacklinksInspector::new(Arc::clone(&core), workspace_root);
        let history = HistoryInspector::new(Arc::clone(&core));
        let graph = NativeGraphView::new(Arc::clone(&core));
        let settings = NativeSettingsController::new(Arc::clone(&core));
        let templates = NativeTemplateService::new(Arc::clone(&core));
        let trash = NativeTrashController::new(Arc::clone(&core), workspace_root);
        let wikixiv = WikixivInspector::new(Arc::clone(&core));
        let book_library = BookLibraryController::new(Arc::clone(&core));
        let _ = trash.run_retention_cleanup(now_ms);

        Ok(Self {
            core,
            workspace_root: workspace_root.to_path_buf(),
            tabs,
            explorer,
            search,
            backlinks,
            history,
            graph,
            settings,
            templates,
            trash,
            wikixiv,
            book_library,
            ui_scale: UiScaleState::default(),
            editors: HashMap::new(),
            editor_lru: VecDeque::new(),
            readers: HashMap::new(),
        })
    }

    pub fn core(&self) -> &Arc<Core> {
        &self.core
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    pub fn theme_palette(&self) -> ThemePalette {
        let cfg = self.settings.config();
        ThemePalette::resolve(&cfg.ui.theme, &cfg.ui.primary_color)
    }

    pub fn locale(&self) -> Locale {
        Locale::from_code(&self.settings.config().ui.language)
    }

    /// Opens a Markdown note in the tab manager and activates its `NativeEditorSession`,
    /// enforcing the configured `editor.live_tabs` memory pool limit.
    pub fn open_note(&mut self, relative_path: &str, in_new_tab: bool) -> Result<Uuid, String> {
        let normalized = relative_path.replace('\\', "/");
        let tab_id = self.tabs.open_document_tab(&normalized, in_new_tab)?;
        self.ensure_editor_loaded(&normalized)?;
        Ok(tab_id)
    }

    pub fn active_editor(&self) -> Option<&NativeEditorSession> {
        let rel = self.tabs.active_tab()?.relative_path.as_deref()?;
        self.editors.get(rel)
    }

    pub fn active_editor_mut(&mut self) -> Option<&mut NativeEditorSession> {
        let rel = self.tabs.active_tab()?.relative_path.clone()?;
        self.editors.get_mut(&rel)
    }

    /// Opens an EPUB or FB2 book in `NativeBookReader`, optionally jumping to a specific CFI.
    pub fn open_book(&mut self, relative_path: &str, cfi: Option<&str>) -> Result<&mut NativeBookReader, String> {
        let normalized = relative_path.replace('\\', "/");
        if !self.readers.contains_key(&normalized) {
            let full_path = self.workspace_root.join(&normalized);
            let reader = NativeBookReader::open(
                Arc::clone(&self.core),
                self.tabs.workspace_id(),
                &self.workspace_root,
                &full_path,
            )?;
            self.readers.insert(normalized.clone(), reader);
        }
        let reader = self.readers.get_mut(&normalized).expect("reader inserted");
        if let Some(target_cfi) = cfi {
            reader.jump_to_cfi(target_cfi);
        }
        Ok(reader)
    }

    pub fn reader(&self, relative_path: &str) -> Option<&NativeBookReader> {
        self.readers.get(&relative_path.replace('\\', "/"))
    }

    /// Follows the `[[WikiLink]]` or `aquilum-reader://` quote link under the active editor's cursor.
    pub fn follow_link_at_cursor(&mut self, in_new_tab: bool) -> Result<Option<InteractiveLinkTarget>, String> {
        let Some(editor) = self.active_editor() else {
            return Ok(None);
        };
        let Some(target) = editor.resolve_link_at_cursor(&self.workspace_root) else {
            return Ok(None);
        };

        match &target {
            InteractiveLinkTarget::WikiNote {
                raw_target,
                resolved_path,
                ..
            } => {
                let rel_path = if let Some(abs_or_rel) = resolved_path {
                    Path::new(abs_or_rel)
                        .strip_prefix(&self.workspace_root)
                        .unwrap_or_else(|_| Path::new(abs_or_rel))
                        .to_string_lossy()
                        .replace('\\', "/")
                } else {
                    let filename = if raw_target.ends_with(".md") {
                        raw_target.clone()
                    } else {
                        format!("{raw_target}.md")
                    };
                    let _ = self.explorer.create_note(&filename, &format!("# {raw_target}\n\n"));
                    filename
                };
                self.open_note(&rel_path, in_new_tab)?;
            }
            InteractiveLinkTarget::ReaderQuote { book_path, cfi } => {
                self.open_book(book_path, Some(cfi))?;
            }
        }
        Ok(Some(target))
    }

    /// Routes a `CoreEvent` emitted by `aquilum-core` (e.g., from MCP tools or file watcher)
    /// into the native shell.
    pub fn handle_core_event(&mut self, event: CoreEvent) -> Result<(), String> {
        match event {
            CoreEvent::DocumentChanged(ev) => {
                let rel = self.to_relative(&ev.path);
                if let Some(editor) = self.editors.get_mut(&rel) {
                    editor.sync_external().map_err(|e| e.to_string())?;
                }
            }
            CoreEvent::NotesRelocated(reloc) => {
                for mv in reloc.moves {
                    let from_rel = self.to_relative(&mv.from);
                    let to_rel = self.to_relative(&mv.to);
                    self.tabs.nav_history.relocate(&from_rel, &to_rel);
                    if let Some(editor) = self.editors.remove(&from_rel) {
                        editor.close();
                        let _ = self.ensure_editor_loaded(&to_rel);
                    }
                }
            }
            CoreEvent::Navigate(nav) => match nav {
                Navigation::Note { path, disposition } => {
                    let rel = self.to_relative(&path);
                    let new_tab = matches!(disposition, Disposition::NewTab);
                    self.open_note(&rel, new_tab)?;
                }
                Navigation::Workspace { .. } => {}
            },
            _ => {}
        }
        Ok(())
    }

    /// Persists all open editor tabs, sticky Yrs cursor anchors, and graph camera to SQLite.
    pub fn save_session(&mut self, now_ms: i64) -> Result<bool, String> {
        let mut cursors = Vec::new();
        for (rel, editor) in &mut self.editors {
            let _ = editor.save_now();
            let sel = editor.selection();
            let vp = editor.viewport();
            let anchor_u16 = editor.rope().char_to_utf16(sel.anchor);
            let head_u16 = editor.rope().char_to_utf16(sel.head);
            let scroll_u16 = editor.rope().char_to_utf16(vp.start_char);
            cursors.push((rel.clone(), anchor_u16, head_u16, scroll_u16));
        }
        self.tabs.save_state_batch(&cursors, now_ms)
    }

    pub fn shutdown(mut self, now_ms: i64) {
        let _ = self.save_session(now_ms);
        for (_, editor) in self.editors.drain() {
            editor.close();
        }
        self.core.shutdown();
    }

    fn ensure_editor_loaded(&mut self, relative_path: &str) -> Result<(), String> {
        let normalized = relative_path.replace('\\', "/");
        if self.editors.contains_key(&normalized) {
            self.editor_lru.retain(|k| k != &normalized);
            self.editor_lru.push_back(normalized);
            return Ok(());
        }

        let live_limit = self.core.settings.get_config().editor.live_tabs.max(1) as usize;
        while self.editors.len() >= live_limit {
            let Some(oldest) = self.editor_lru.pop_front() else { break };
            if let Some(evicted) = self.editors.get_mut(&oldest) {
                if let Err(error) = evicted.save_now() {
                    // Never drop the only live editor copy when disk persistence failed.
                    self.editor_lru.push_front(oldest);
                    return Err(format!("cannot evict unsaved editor: {error}"));
                }
            }
            if let Some(evicted) = self.editors.remove(&oldest) {
                evicted.close();
            }
        }

        let full_path = self.workspace_root.join(&normalized);
        let session = NativeEditorSession::open(Arc::clone(&self.core), &full_path, "aquilum-native-shell")
            .map_err(|e| e.to_string())?;
        self.editors.insert(normalized.clone(), session);
        self.editor_lru.push_back(normalized);
        Ok(())
    }

    fn to_relative(&self, path: &str) -> String {
        Path::new(path)
            .strip_prefix(&self.workspace_root)
            .unwrap_or_else(|_| Path::new(path))
            .to_string_lossy()
            .replace('\\', "/")
    }
}
