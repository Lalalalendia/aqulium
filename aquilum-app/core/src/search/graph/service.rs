use super::encode::encode;
use super::snapshot::{paths_at, RenderSnapshot};
use crate::search::error::SearchError;
use crate::search::models::SearchIndexState;
use crate::search::paths::same_workspace;
use crate::search::service::{CachedRenderGraph, SearchService};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

struct GraphSource {
    root: PathBuf,
    generation: u64,
    revision: u64,
    metadata_path: PathBuf,
}

impl SearchService {
    pub fn render_graph(&self, workspace: &str) -> Result<Arc<Vec<u8>>, SearchError> {
        let source = self.graph_source(workspace)?;
        if let Some(ready) = self.cached_render_graph(&source)? {
            return Ok(ready);
        }
        let _building = self.render_lock.lock().map_err(SearchError::task)?;
        if let Some(ready) = self.cached_render_graph(&source)? {
            return Ok(ready);
        }
        let snapshot = RenderSnapshot::build(
            &Connection::open(&source.metadata_path)?,
            &source.root,
        )?;
        let epoch = self.next_render_epoch.fetch_add(1, Ordering::Relaxed) + 1;
        let bytes = Arc::new(encode(&snapshot, epoch));
        let paths = Arc::new(snapshot.paths);
        let mut cache = self.render_graph.write().map_err(SearchError::task)?;
        *cache = Some(CachedRenderGraph {
            root: source.root,
            generation: source.generation,
            revision: source.revision,
            epoch,
            paths,
            bytes: Arc::clone(&bytes),
        });
        Ok(bytes)
    }

    pub fn graph_paths(&self, epoch: u64, indices: &[u32]) -> Result<Vec<String>, SearchError> {
        let cache = self.render_graph.read().map_err(SearchError::task)?;
        let cached = cache.as_ref().filter(|cached| cached.epoch == epoch).ok_or_else(|| {
            SearchError::Unavailable {
                message: "Снимок графа устарел".to_owned(),
            }
        })?;
        Ok(paths_at(&cached.paths, indices))
    }

    fn graph_source(&self, workspace: &str) -> Result<GraphSource, SearchError> {
        let active = self.active.read().map_err(SearchError::task)?;
        let active = active.as_ref().ok_or_else(|| SearchError::Unavailable {
            message: "Индекс ещё не готов".to_owned(),
        })?;
        if !same_workspace(&active.root, workspace) {
            return Err(SearchError::InvalidWorkspace {
                message: "Индекс принадлежит другой базе знаний".to_owned(),
            });
        }
        let status = active.progress.snapshot();
        if status.state == SearchIndexState::Error {
            return Err(SearchError::Unavailable {
                message: status
                    .error
                    .unwrap_or_else(|| "Поисковый индекс недоступен".to_owned()),
            });
        }
        if status.updating {
            return Err(SearchError::Unavailable {
                message: "Индекс обновляется".to_owned(),
            });
        }
        Ok(GraphSource {
            root: active.root.clone(),
            generation: active.generation,
            revision: status.revision,
            metadata_path: active.metadata_path.clone(),
        })
    }

    fn cached_render_graph(
        &self,
        source: &GraphSource,
    ) -> Result<Option<Arc<Vec<u8>>>, SearchError> {
        let cache = self.render_graph.read().map_err(SearchError::task)?;
        Ok(cache.as_ref().and_then(|cached| {
            (cached.root == source.root
                && cached.generation == source.generation
                && cached.revision == source.revision)
                .then(|| Arc::clone(&cached.bytes))
        }))
    }
}
