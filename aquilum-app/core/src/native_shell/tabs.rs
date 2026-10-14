use crate::app_core::Core;
use crate::ui_state::models::{
    GraphCameraState, OpenSessionInput, SaveStateBatchInput, SessionStateInput, TabKind, TabState,
    ViewStateInput,
};
use crate::ui_state::paths::{canonical_workspace, normalize_relative};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeTab {
    pub tab_id: Uuid,
    pub document_id: Option<Uuid>,
    pub kind: TabKind,
    pub relative_path: Option<String>,
    pub title: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NavigationHistory {
    back: Vec<String>,
    forward: Vec<String>,
    current: Option<String>,
}

impl NavigationHistory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> Option<&str> {
        self.current.as_deref()
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    pub fn visit(&mut self, relative_path: impl Into<String>) {
        let next = relative_path.into();
        if self.current.as_deref() == Some(next.as_str()) {
            return;
        }
        if let Some(prev) = self.current.replace(next) {
            self.back.push(prev);
        }
        self.forward.clear();
    }

    pub fn go_back(&mut self) -> Option<String> {
        let target = self.back.pop()?;
        if let Some(cur) = self.current.replace(target.clone()) {
            self.forward.push(cur);
        }
        Some(target)
    }

    pub fn go_forward(&mut self) -> Option<String> {
        let target = self.forward.pop()?;
        if let Some(cur) = self.current.replace(target.clone()) {
            self.back.push(cur);
        }
        Some(target)
    }

    pub fn relocate(&mut self, from_rel: &str, to_rel: &str) {
        for item in &mut self.back {
            if item == from_rel {
                *item = to_rel.to_owned();
            }
        }
        for item in &mut self.forward {
            if item == from_rel {
                *item = to_rel.to_owned();
            }
        }
        if self.current.as_deref() == Some(from_rel) {
            self.current = Some(to_rel.to_owned());
        }
    }
}

pub struct NativeTabManager {
    core: Arc<Core>,
    workspace_id: Uuid,
    workspace_root: PathBuf,
    window_id: String,
    epoch: Uuid,
    sequence: i64,
    tabs: Vec<NativeTab>,
    active_tab_id: Option<Uuid>,
    pub nav_history: NavigationHistory,
    pub graph_camera: Option<GraphCameraState>,
}

impl NativeTabManager {
    pub fn open(
        core: Arc<Core>,
        workspace_root: &Path,
        window_id: impl Into<String>,
        now_ms: i64,
    ) -> Result<Self, String> {
        let window_id = window_id.into();
        let canonical_ws = canonical_workspace(&workspace_root.to_string_lossy())
            .unwrap_or_else(|_| workspace_root.to_string_lossy().into_owned());
        let workspace_id = core
            .ui_state
            .resolve_workspace(&canonical_ws, now_ms)
            .map_err(|e| format!("resolve_workspace failed: {e:?}"))?;
        let epoch = Uuid::new_v4();
        let loaded = core
            .ui_state
            .open_session(&OpenSessionInput {
                workspace_id,
                window_id: window_id.clone(),
                epoch,
                now_ms,
            })
            .map_err(|e| format!("open_session failed: {e:?}"))?;

        let mut tabs: Vec<NativeTab> = loaded
            .tabs
            .into_iter()
            .map(|t| {
                let title = t
                    .relative_path
                    .as_deref()
                    .and_then(|p| Path::new(p).file_stem())
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| match t.kind {
                        TabKind::Graph => "Граф знаний".to_owned(),
                        TabKind::Empty => "Новая вкладка".to_owned(),
                        TabKind::Document => "Заметка".to_owned(),
                    });
                NativeTab {
                    tab_id: t.tab_id,
                    document_id: t.document_id,
                    kind: t.kind,
                    relative_path: t.relative_path,
                    title,
                }
            })
            .collect();

        let mut active_tab_id = loaded.active_tab_id;
        if tabs.is_empty() {
            let empty_id = Uuid::new_v4();
            tabs.push(NativeTab {
                tab_id: empty_id,
                document_id: None,
                kind: TabKind::Empty,
                relative_path: None,
                title: "Новая вкладка".to_owned(),
            });
            active_tab_id = Some(empty_id);
        }

        let mut nav_history = NavigationHistory::new();
        if let Some(active) = active_tab_id.and_then(|id| tabs.iter().find(|t| t.tab_id == id)) {
            if let Some(rel) = active.relative_path.as_deref() {
                nav_history.visit(rel);
            }
        }

        Ok(Self {
            core,
            workspace_id,
            workspace_root: workspace_root.to_path_buf(),
            window_id,
            epoch,
            sequence: 0,
            tabs,
            active_tab_id,
            nav_history,
            graph_camera: loaded.graph_camera,
        })
    }

    pub fn workspace_id(&self) -> Uuid {
        self.workspace_id
    }

    pub fn tabs(&self) -> &[NativeTab] {
        &self.tabs
    }

    pub fn active_tab(&self) -> Option<&NativeTab> {
        let id = self.active_tab_id?;
        self.tabs.iter().find(|t| t.tab_id == id)
    }

    pub fn active_tab_id(&self) -> Option<Uuid> {
        self.active_tab_id
    }

    pub fn open_document_tab(&mut self, relative_path: &str, in_new_tab: bool) -> Result<Uuid, String> {
        let normalized = normalize_relative(relative_path)
            .unwrap_or_else(|_| relative_path.replace('\\', "/"));
        if let Some(existing) = self
            .tabs
            .iter()
            .find(|t| t.kind == TabKind::Document && t.relative_path.as_deref() == Some(normalized.as_str()))
        {
            let id = existing.tab_id;
            self.active_tab_id = Some(id);
            self.nav_history.visit(&normalized);
            return Ok(id);
        }

        let doc_id = self
            .core
            .ui_state
            .resolve_document(self.workspace_id, &normalized)
            .map_err(|e| format!("resolve_document failed: {e:?}"))?;
        let title = Path::new(&normalized)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| normalized.clone());

        if !in_new_tab {
            if let Some(active_id) = self.active_tab_id {
                if let Some(slot) = self.tabs.iter_mut().find(|t| t.tab_id == active_id) {
                    slot.document_id = Some(doc_id);
                    slot.kind = TabKind::Document;
                    slot.relative_path = Some(normalized.clone());
                    slot.title = title;
                    self.nav_history.visit(&normalized);
                    return Ok(active_id);
                }
            }
        }

        let tab_id = Uuid::new_v4();
        self.tabs.push(NativeTab {
            tab_id,
            document_id: Some(doc_id),
            kind: TabKind::Document,
            relative_path: Some(normalized.clone()),
            title,
        });
        self.active_tab_id = Some(tab_id);
        self.nav_history.visit(&normalized);
        Ok(tab_id)
    }

    pub fn open_graph_tab(&mut self) -> Uuid {
        if let Some(existing) = self.tabs.iter().find(|t| t.kind == TabKind::Graph) {
            let id = existing.tab_id;
            self.active_tab_id = Some(id);
            return id;
        }
        let tab_id = Uuid::new_v4();
        self.tabs.push(NativeTab {
            tab_id,
            document_id: None,
            kind: TabKind::Graph,
            relative_path: None,
            title: "Граф знаний".to_owned(),
        });
        self.active_tab_id = Some(tab_id);
        tab_id
    }

    pub fn reorder_tab(&mut self, from_index: usize, to_index: usize) {
        if from_index >= self.tabs.len() || to_index >= self.tabs.len() || from_index == to_index {
            return;
        }
        let tab = self.tabs.remove(from_index);
        self.tabs.insert(to_index, tab);
    }

    pub fn close_tab(&mut self, tab_id: Uuid) {
        let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) else {
            return;
        };
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            let empty_id = Uuid::new_v4();
            self.tabs.push(NativeTab {
                tab_id: empty_id,
                document_id: None,
                kind: TabKind::Empty,
                relative_path: None,
                title: "Новая вкладка".to_owned(),
            });
            self.active_tab_id = Some(empty_id);
        } else if self.active_tab_id == Some(tab_id) {
            let next_idx = idx.min(self.tabs.len() - 1);
            self.active_tab_id = Some(self.tabs[next_idx].tab_id);
        }
    }

    /// Saves the current tab bar, graph camera, and per-document sticky Yrs cursor positions
    /// (`encode_positions`) into `ui-state.sqlite3`.
    pub fn save_state_batch(
        &mut self,
        cursor_utf16_by_rel_path: &[(String, usize, usize, usize)],
        now_ms: i64,
    ) -> Result<bool, String> {
        self.sequence += 1;
        let tab_states: Vec<TabState> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| TabState {
                tab_id: t.tab_id,
                document_id: t.document_id,
                kind: t.kind.clone(),
                position: i as i64,
            })
            .collect();

        let mut views = Vec::new();
        for (rel_path, anchor_u16, head_u16, scroll_u16) in cursor_utf16_by_rel_path {
            let Some(tab) = self
                .tabs
                .iter()
                .find(|t| t.relative_path.as_deref() == Some(rel_path.as_str()))
            else {
                continue;
            };
            let Some(document_id) = tab.document_id else { continue };
            let abs_path = self.workspace_root.join(rel_path);
            let encoded = self
                .core
                .documents
                .encode_positions(&abs_path, &[*anchor_u16, *head_u16, *scroll_u16]);
            let (c_anchor, c_head, s_anchor) = match encoded {
                Some(mut v) if v.len() == 3 => {
                    let s = v.pop().flatten().unwrap_or_default();
                    let h = v.pop().flatten().unwrap_or_default();
                    let a = v.pop().flatten().unwrap_or_default();
                    (a, h, s)
                }
                _ => (Vec::new(), Vec::new(), Vec::new()),
            };
            views.push(ViewStateInput {
                path: Some(abs_path.to_string_lossy().into_owned()),
                document_id,
                pane_id: "main".to_owned(),
                cursor_anchor: c_anchor,
                cursor_head: c_head,
                fallback_anchor: *anchor_u16 as i64,
                fallback_head: *head_u16 as i64,
                scroll_anchor: s_anchor,
                fallback_scroll_anchor: *scroll_u16 as i64,
                scroll_offset_px: 0.0,
                focused_surface: "editor".to_owned(),
            });
        }

        let input = SaveStateBatchInput {
            workspace_id: self.workspace_id,
            window_id: self.window_id.clone(),
            epoch: self.epoch,
            sequence: self.sequence,
            now_ms,
            session: Some(SessionStateInput {
                active_tab_id: self.active_tab_id,
                tabs: Some(tab_states),
            }),
            views,
            graph_camera: self.graph_camera,
        };

        self.core
            .ui_state
            .save_batch(&input)
            .map_err(|e| format!("save_batch failed: {e:?}"))
    }
}
