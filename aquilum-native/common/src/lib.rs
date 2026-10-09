//! Shared native host for Strangler Fig prototypes.
//! This uses the real Aquilum core: SQLite, Yrs CRDT, history and settings.
//! No Tauri or web frontend is involved. No copy of the backend.
//! WARNING: prototype with explicit Save only, not suitable for valuable notes.

pub mod text_positions;

use aquilum_core::app_core::{Core, CoreEvent, EventSink};
use aquilum_core::history::Source;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug)]
pub enum NativeError {
    Io(io::Error),
    Core(String),
}
impl fmt::Display for NativeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Core(error) => f.write_str(error),
        }
    }
}
impl std::error::Error for NativeError {}
impl From<io::Error> for NativeError {
    fn from(error: io::Error) -> Self { Self::Io(error) }
}

#[derive(Clone)]
pub struct NativeHost {
    core: Arc<Core>,
}
impl NativeHost {
    /// Pass a dedicated lab data directory, never Aquilum's real profile.
    pub fn new(data_dir: &Path) -> Result<Self, NativeError> {
        fs::create_dir_all(data_dir)?;
        let sink: Arc<dyn EventSink> = Arc::new(|_event: CoreEvent| {});
        Ok(Self { core: Core::open(data_dir, sink) })
    }

    pub fn open(&self, path: impl AsRef<Path>) -> Result<NativeDocument, NativeError> {
        let path = path.as_ref().canonicalize()?;
        if !path.is_file() {
            return Err(NativeError::Core(format!("Not a file: {}", path.display())));
        }
        let opened = self.core.documents.open(&self.core, &path, 0)
            .map_err(|e| NativeError::Core(format!("{e:?}")))?;
        Ok(NativeDocument {
            core: Arc::clone(&self.core),
            path,
            version: opened.version,
            initial_text: opened.text,
        })
    }

    pub fn shutdown(&self) { self.core.shutdown(); }
}

/// Holds a DocumentHub session open and releases it on drop.
/// Save uses Aquilum's real conflict/version rules.
pub struct NativeDocument {
    core: Arc<Core>,
    path: PathBuf,
    version: u64,
    initial_text: String,
}
impl NativeDocument {
    pub fn path(&self) -> &Path { &self.path }
    pub fn version(&self) -> u64 { self.version }
    pub fn initial_text(&self) -> &str { &self.initial_text }
    /// Transfer initial text ownership to UI; avoids keeping an extra copy.
    pub fn take_initial_text(&mut self) -> String { std::mem::take(&mut self.initial_text) }

    pub fn save(&mut self, text: &str) -> Result<u64, NativeError> {
        let next = self.core.documents.replace_text(
            &self.core, &self.path, text, "aquilum-native-lab",
            Source::Me, Some(self.version),
        ).map_err(|e| NativeError::Core(format!("{e:?}")))?;
        self.version = next;
        Ok(next)
    }

    /// Send ONE CodeMirror-compatible ChangeSet JSON batch to the real
    /// DocumentHub / Yrs replica. The base version is checked by the same
    /// kernel path that the TypeScript editor already uses.
    ///
    /// The caller must create and validate the change before calling this
    /// function, then commit its local UI/rope model only on success. This is
    /// NOT a durability/Undo-Redo guarantee; desktop rollback and fsync are
    /// later acceptance gates.
    pub fn push_json_change(&mut self, change: serde_json::Value) -> Result<u64, NativeError> {
        let accepted = self.core.documents.push(
            &self.core, &self.path, self.version, "aquilum-native-rope",
            std::slice::from_ref(&change),
        ).map_err(|e| NativeError::Core(format!("{e:?}")))?;
        if !accepted {
            return Err(NativeError::Core(format!(
                "DocumentHub rejected stale edit (expected version {})",
                self.version,
            )));
        }
        self.version = self.version.checked_add(1).ok_or_else(|| {
            NativeError::Core("DocumentHub version counter overflow".to_owned())
        })?;
        Ok(self.version)
    }

    pub fn refresh(&mut self) -> Result<String, NativeError> {
        let latest = self.core.documents.read(&self.core, &self.path)
            .map_err(|e| NativeError::Core(format!("{e:?}")))?;
        self.version = latest.version;
        Ok(latest.text)
    }
}
impl Drop for NativeDocument {
    fn drop(&mut self) {
        self.core.documents.release(&self.core, &self.path);
    }
}

/// Command line fixtures use --file PATH repeatedly. Do not create files
/// implicitly or touch a real vault unless the user supplies its path.
pub fn lab_file_paths() -> Result<Vec<PathBuf>, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut files = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] != "--file" || i + 1 >= args.len() {
            return Err("Usage: program --file PATH [--file PATH ...]".to_owned());
        }
        files.push(PathBuf::from(&args[i + 1]));
        i += 2;
    }
    if files.is_empty() { return Err("Pass at least one --file PATH".to_owned()); }
    Ok(files)
}
pub fn lab_profile_dir() -> PathBuf {
    std::env::temp_dir().join("aquilum-native-lab").join(std::process::id().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_saves_and_reopens_via_real_document_hub() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("Заметка.md");
        fs::write(&file, "Привет 🌍\n# Header\n").unwrap();
        let host = NativeHost::new(&temp.path().join("profile")).unwrap();
        let mut first = host.open(&file).unwrap();
        assert_eq!(first.initial_text(), "Привет 🌍\n# Header\n");
        assert_eq!(first.take_initial_text(), "Привет 🌍\n# Header\n");
        assert!(first.initial_text().is_empty());
        first.save("Привет 🌍\nТест UTF-16 👍🏼\n").unwrap();
        drop(first);
        host.shutdown();
        assert_eq!(fs::read_to_string(&file).unwrap(), "Привет 🌍\nТест UTF-16 👍🏼\n");
        let again = NativeHost::new(&temp.path().join("profile")).unwrap();
        let doc = again.open(&file).unwrap();
        assert_eq!(doc.initial_text(), "Привет 🌍\nТест UTF-16 👍🏼\n");
    }

    #[test]
    fn stale_version_does_not_overwrite_another_session() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("shared.md");
        fs::write(&file, "original").unwrap();
        let host = NativeHost::new(&temp.path().join("profile")).unwrap();
        let mut first = host.open(&file).unwrap();
        let mut second = host.open(&file).unwrap();
        first.save("winner").unwrap();
        assert!(second.save("loser").is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(), "winner");
        assert_eq!(second.refresh().unwrap(), "winner");
        assert!(second.save("winner plus").is_ok());
        drop(first);
        drop(second);
        host.shutdown();
        assert_eq!(fs::read_to_string(&file).unwrap(), "winner plus");
    }

    #[test]
    fn failed_open_never_creates_an_input_file() {
        let temp = tempfile::tempdir().unwrap();
        let host = NativeHost::new(&temp.path().join("profile")).unwrap();
        let path = temp.path().join("nonexistent.md");
        assert!(host.open(&path).is_err());
        assert!(!path.exists());
    }
}
