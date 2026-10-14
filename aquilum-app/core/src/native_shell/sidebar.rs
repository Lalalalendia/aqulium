use crate::app_core::Core;
use crate::files::error::FileCommandError;
use crate::files::gate;
use crate::files::models::{FileItem, FileRenameResult, FileWriteResult};
use crate::files::workspace::read_directory_impl;
use crate::history::service::{HistoryPage, VersionTexts};
use crate::history::Source;
use crate::search::analysis::models::{AnalysisMethod, AnalysisResult};
use crate::search::error::SearchError;
use crate::search::models::{Backlink, NoteSuggestion, OutgoingLink, SearchResponse};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct WorkspaceExplorer {
    core: Arc<Core>,
    workspace_root: PathBuf,
    expanded_folders: BTreeSet<String>,
}

impl WorkspaceExplorer {
    pub fn new(core: Arc<Core>, workspace_root: &Path) -> Self {
        Self {
            core,
            workspace_root: workspace_root.to_path_buf(),
            expanded_folders: BTreeSet::new(),
        }
    }

    pub fn list_directory(&self, dir: &Path) -> Result<Vec<FileItem>, FileCommandError> {
        read_directory_impl(dir)
    }

    pub fn list_root(&self) -> Result<Vec<FileItem>, FileCommandError> {
        read_directory_impl(&self.workspace_root)
    }

    pub fn toggle_folder(&mut self, folder_id: &str) -> bool {
        if self.expanded_folders.contains(folder_id) {
            self.expanded_folders.remove(folder_id);
            false
        } else {
            self.expanded_folders.insert(folder_id.to_owned());
            true
        }
    }

    pub fn is_expanded(&self, folder_id: &str) -> bool {
        self.expanded_folders.contains(folder_id)
    }

    pub fn create_note(&self, relative_path: &str, initial_content: &str) -> Result<FileWriteResult, FileCommandError> {
        let full = self.workspace_root.join(relative_path);
        gate::create(&self.core, &full, initial_content, Source::Me, None)
    }

    pub fn rename_note(&self, from_rel: &str, to_rel: &str) -> Result<FileRenameResult, FileCommandError> {
        let from = self.workspace_root.join(from_rel);
        let to = self.workspace_root.join(to_rel);
        gate::rename(&self.core, &from, &to)
    }

    pub fn trash_note(&self, relative_path: &str) -> Result<(), FileCommandError> {
        let full = self.workspace_root.join(relative_path);
        gate::trash(&self.core, &self.workspace_root, &full).map(|_| ())
    }
}

pub struct GlobalSearchController {
    core: Arc<Core>,
    workspace_root: PathBuf,
}

impl GlobalSearchController {
    pub fn new(core: Arc<Core>, workspace_root: &Path) -> Self {
        Self {
            core,
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<SearchResponse, SearchError> {
        self.core
            .search
            .search(&self.workspace_root.to_string_lossy(), query, limit)
    }

    pub fn suggest(&self, query: &str, limit: usize) -> Result<Vec<NoteSuggestion>, SearchError> {
        self.core
            .search
            .suggest_notes(&self.workspace_root.to_string_lossy(), query, limit)
    }
}

#[derive(Clone, Debug)]
pub struct BacklinksSnapshot {
    pub backlinks: Vec<Backlink>,
    pub outgoing_links: Vec<OutgoingLink>,
    pub bm25f_related: Vec<AnalysisResult>,
    pub adamic_adar_related: Vec<AnalysisResult>,
}

pub struct BacklinksInspector {
    core: Arc<Core>,
    workspace_root: PathBuf,
}

impl BacklinksInspector {
    pub fn new(core: Arc<Core>, workspace_root: &Path) -> Self {
        Self {
            core,
            workspace_root: workspace_root.to_path_buf(),
        }
    }

    pub fn inspect(&self, document_path: &Path, limit: Option<usize>) -> Result<BacklinksSnapshot, SearchError> {
        let ws = self.workspace_root.to_string_lossy();
        let doc = document_path.to_string_lossy();
        let config = self.core.settings.get_config();

        let backlinks = self.core.search.backlinks(&ws, &doc)?;
        let outgoing_links = self.core.search.outgoing_links(&ws, &doc)?;
        let bm25f_related = if config.analysis.enable_bm25f {
            self.core
                .search
                .analyze_document(&ws, &doc, AnalysisMethod::Bm25f, limit, &config)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let adamic_adar_related = if config.analysis.enable_adamic_adar {
            self.core
                .search
                .analyze_document(&ws, &doc, AnalysisMethod::AdamicAdar, limit, &config)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(BacklinksSnapshot {
            backlinks,
            outgoing_links,
            bm25f_related,
            adamic_adar_related,
        })
    }
}

pub struct HistoryInspector {
    core: Arc<Core>,
}

impl HistoryInspector {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }

    pub fn page(&self, note_path: &Path, offset: usize, limit: usize) -> HistoryPage {
        self.core
            .history
            .page(&|| self.core.known_roots(), note_path, offset, limit.min(200))
    }

    pub fn version(&self, note_path: &Path, version_id: Option<&str>) -> Option<VersionTexts> {
        self.core
            .history
            .version(&|| self.core.known_roots(), note_path, version_id)
    }

    pub fn name_version(&self, note_path: &Path, version_id: Option<&str>, name: &str) -> Option<String> {
        gate::name_version(&self.core, note_path, version_id, name)
    }

    pub fn revert_to_version(&self, note_path: &Path, version_id: Option<&str>) -> Result<bool, String> {
        let Some(texts) = self.version(note_path, version_id) else {
            return Err("version not found".to_owned());
        };
        let previous = texts.previous.as_deref().unwrap_or(&texts.text);
        self.core
            .documents
            .revert(&self.core, note_path, &texts.text, previous, Source::Me)
            .map_err(|e| format!("revert failed: {e:?}"))
    }
}
