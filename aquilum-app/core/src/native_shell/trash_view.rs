//! Native Trash Manager (`DeletionPage`, `TrashState`, Restore, Empty Trash),
//! Automatic Retention Cleanup (Trash + History + UI State), and Wikixiv Thematic Inspector.

use crate::app_core::Core;
use crate::files::deletions::{deletion_page_impl, DeletionPage};
use crate::files::error::FileCommandError;
use crate::files::gate;
use crate::files::trash::{
    cleanup_trash_impl, empty_trash_impl, list_trash_impl, trash_path, trash_state_impl,
    TrashState, TrashedFile,
};
use crate::history::store as history_store;
use crate::wikixiv::models::{SearchRequest, WikixivSearchResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetentionCleanupSummary {
    pub removed_trash_files: u64,
    pub removed_history_versions: u64,
    pub removed_ui_state_rows: usize,
}

pub struct NativeTrashController {
    core: Arc<Core>,
    workspace_root: PathBuf,
}

impl NativeTrashController {
    pub fn new(core: Arc<Core>, workspace_root: &Path) -> Self {
        Self {
            core,
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    pub fn state(&self) -> Result<TrashState, FileCommandError> {
        trash_state_impl(&self.workspace_root)
    }

    pub fn deletions_page(&self, offset: usize, limit: usize) -> Result<DeletionPage, FileCommandError> {
        deletion_page_impl(&self.workspace_root, offset, limit)
    }

    pub fn list_files(&self) -> Result<Vec<TrashedFile>, FileCommandError> {
        list_trash_impl(&self.workspace_root)
    }

    pub fn restore_deletion(&self, deletion_id: &str) -> Result<(), FileCommandError> {
        gate::restore_deletion(&self.core, &self.workspace_root, deletion_id)
    }

    pub fn empty_trash(&self) -> Result<u64, FileCommandError> {
        empty_trash_impl(&self.workspace_root)
    }

    /// Runs Aquilum's startup retention cleanup across `.trash`, `.aquilum/history`,
    /// and `ui-state.sqlite3` according to `AppConfig`.
    pub fn run_retention_cleanup(&self, now_ms: i64) -> RetentionCleanupSummary {
        let cfg = self.core.settings.get_config();
        let removed_trash_files =
            cleanup_trash_impl(&self.workspace_root, cfg.trash.retention_days).unwrap_or(0);

        history_store::adopt_quantum_history(&self.workspace_root);
        history_store::adopt_quantum_history(&trash_path(&self.workspace_root));
        let removed_history_versions =
            history_store::remove_expired(&self.workspace_root, cfg.history.retention_days);

        let removed_ui_state_rows = self
            .core
            .ui_state
            .cleanup(cfg.trash.retention_days, now_ms)
            .unwrap_or(0);

        RetentionCleanupSummary {
            removed_trash_files,
            removed_history_versions,
            removed_ui_state_rows,
        }
    }
}

pub struct WikixivInspector {
    core: Arc<Core>,
}

impl WikixivInspector {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }

    pub fn search_for_note(
        &self,
        document_path: Option<&Path>,
        note_text: &str,
        generation: u64,
    ) -> Result<WikixivSearchResult, String> {
        let enabled = self.core.settings.get_config().analysis.enable_wikixiv;
        self.core.wikixiv.search(
            SearchRequest {
                text: note_text.to_owned(),
                document_path: document_path.map(|p| p.to_string_lossy().into_owned()),
                generation,
            },
            enabled,
        )
    }
}
