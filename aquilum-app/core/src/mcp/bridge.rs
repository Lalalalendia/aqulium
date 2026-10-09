use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use crate::app_core::{Core, CoreEvent};

const AUTO_OPEN_COOLDOWN: Duration = Duration::from_secs(5);
static LAST_AUTO_OPEN: Mutex<Option<Instant>> = Mutex::new(None);

pub fn open_note_after_write(core: &Core, path: &Path) {
    let mut last = LAST_AUTO_OPEN.lock().unwrap_or_else(|error| error.into_inner());
    let now = Instant::now();
    if last.is_some_and(|previous| now.duration_since(previous) < AUTO_OPEN_COOLDOWN) {
        return;
    }
    *last = Some(now);
    open_note(core, path, true);
}

/// Where an MCP tool asks the UI to go. Serialized as the `mcp-navigate` payload the frontend reads:
/// `{"kind":"note","path":…,"disposition":"new-tab"}` or `{"kind":"workspace","path":…}`.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Navigation {
    Note { path: String, disposition: Disposition },
    Workspace { path: String },
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    NewTab,
    Current,
}

pub fn open_note(core: &Core, path: &Path, in_new_tab: bool) {
    let disposition = if in_new_tab { Disposition::NewTab } else { Disposition::Current };
    core.emit(CoreEvent::Navigate(Navigation::Note { path: path.to_string_lossy().into_owned(), disposition }));
}

pub fn switch_workspace(core: &Core, path: &Path) {
    core.emit(CoreEvent::Navigate(Navigation::Workspace { path: path.to_string_lossy().into_owned() }));
}
