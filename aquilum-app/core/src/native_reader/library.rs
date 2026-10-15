//! Book Library, Companion BookPage Frontmatter Synchronization (`pages: "cur/total"`),
//! Book File Importing, and Procedural Cover Patterns (`City`, `Rays`, `Star`, `Geometric`).

use super::NativeBookReader;
use crate::app_core::Core;
use crate::documents::session::SYSTEM_CLIENT;
use crate::files::gate;
use crate::history::Source;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use walkdir::WalkDir;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverPatternKind {
    City,
    Rays,
    Star,
    Geometric,
}

impl CoverPatternKind {
    pub fn for_title(title: &str) -> Self {
        let mut hash: u32 = 2166136261;
        for b in title.as_bytes() {
            hash ^= u32::from(*b);
            hash = hash.wrapping_mul(16777619);
        }
        match hash % 4 {
            0 => Self::City,
            1 => Self::Rays,
            2 => Self::Star,
            _ => Self::Geometric,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookLibraryEntry {
    pub title: String,
    pub author: String,
    pub book_rel_path: String,
    pub companion_note_rel_path: Option<String>,
    pub current_page: usize,
    pub total_pages: usize,
    pub progress_percent: i64,
    pub cfi: Option<String>,
    pub cover_url: Option<String>,
    pub cover_pattern: CoverPatternKind,
}

pub struct BookLibraryController {
    core: Arc<Core>,
}

impl BookLibraryController {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }

    /// Scans the workspace for `.fb2` / `.epub` books and companion BookPage notes,
    /// combining metadata with `UiStateService::load_reader_state`.
    pub fn list_books(&self, workspace_root: &Path, workspace_id: Uuid) -> Vec<BookLibraryEntry> {
        let mut entries = Vec::new();
        for entry in WalkDir::new(workspace_root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            // Only hide files and directories *within* the workspace.
            // An absolute workspace root may itself be inside a hidden
            // directory (e.g. tempfile's `.tmp...`), which must not hide
            // every book returned by the scan.
            let relative_path = path.strip_prefix(workspace_root).unwrap_or(path);
            if relative_path
                .components()
                .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
            {
                continue;
            }
            let Some(ext) = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase())
            else {
                continue;
            };
            if ext != "fb2" && ext != "epub" {
                continue;
            }

            let rel = path
                .strip_prefix(workspace_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");

            let (title, author, total_paras) = match NativeBookReader::open(
                Arc::clone(&self.core),
                workspace_id,
                workspace_root,
                path,
            ) {
                Ok(r) => {
                    let meta = &r.book().metadata;
                    let a = meta.authors.join(", ");
                    (meta.title.clone(), a, r.book().total_paragraphs.max(1))
                }
                Err(_) => (
                    path.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "Book".to_owned()),
                    String::new(),
                    100,
                ),
            };

            let saved = self
                .core
                .ui_state
                .load_reader_state(workspace_id, &rel)
                .ok()
                .flatten();
            let progress_percent = saved.as_ref().map_or(0, |s| s.current.clamp(0, 100));
            let current_page = ((progress_percent as usize) * total_paras) / 100;
            let cfi = saved.and_then(|s| s.cfi);

            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Book".to_owned());
            let candidate_note = workspace_root.join(format!("{stem}.md"));
            let companion_note_rel_path = candidate_note
                .is_file()
                .then(|| format!("{stem}.md"));

            entries.push(BookLibraryEntry {
                 cover_pattern: CoverPatternKind::for_title(&title),
                title,
                author,
                book_rel_path: rel,
                companion_note_rel_path,
                current_page,
                total_pages: total_paras,
                progress_percent,
                cfi,
                cover_url: None,
            });
        }
        entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        entries
    }

    /// Imports an external `.fb2` or `.epub` file into the workspace's `files.folder` (`Files/`)
    /// and creates its companion Markdown BookPage note with `book`, `author`, `pages` frontmatter.
    pub fn import_book_and_create_page(
        &self,
        workspace_root: &Path,
        workspace_id: Uuid,
        source_book_path: &Path,
    ) -> Result<BookLibraryEntry, String> {
        let files_folder = self.core.settings.get_config().files.folder;
        let dest_dir = workspace_root.join(&files_folder);
        fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;

        let file_name = source_book_path
            .file_name()
            .ok_or_else(|| "missing book file name".to_owned())?
            .to_string_lossy()
            .into_owned();
        let dest_book_path = dest_dir.join(&file_name);
        if source_book_path != dest_book_path {
            fs::copy(source_book_path, &dest_book_path).map_err(|e| e.to_string())?;
        }

        let reader = NativeBookReader::open(
            Arc::clone(&self.core),
            workspace_id,
            workspace_root,
            &dest_book_path,
        )?;
        let title = reader.book().metadata.title.clone();
        let author = reader.book().metadata.authors.join(", ");
        let total_pages = reader.book().total_paragraphs.max(1);
        let book_rel = format!("{files_folder}/{file_name}");

        let note_rel = format!("{title}.md");
        let note_full = workspace_root.join(&note_rel);
        let note_content = format!(
            "---\nbook: \"{book_rel}\"\nauthor: \"{author}\"\npages: \"0/{total_pages}\"\nstatus: \"reading\"\n---\n# {title}\n\n> [!book] Конспект книги\n> Автор: **{author}**\n\n"
        );
        if !note_full.exists() {
            gate::create(&self.core, &note_full, &note_content, Source::Me, None)
                .map_err(|e| format!("{e:?}"))?;
        }

        Ok(BookLibraryEntry {
            cover_pattern: CoverPatternKind::for_title(&title),
            title,
            author,
            book_rel_path: book_rel,
            companion_note_rel_path: Some(note_rel),
            current_page: 0,
            total_pages,
            progress_percent: 0,
            cfi: Some(reader.current_cfi()),
            cover_url: None,
        })
    }

    /// Synchronizes reading progress (`pages: "cur/total"`) into the companion BookPage note's
    /// YAML frontmatter via `DocumentHub`.
    pub fn sync_progress_to_companion_note(
        &self,
        note_path: &Path,
        current_page: usize,
        total_pages: usize,
    ) -> Result<bool, String> {
        let opened = self
            .core
            .documents
            .read(&self.core, note_path)
            .map_err(|e| format!("{e:?}"))?;
        let new_pages_line = format!("pages: \"{current_page}/{total_pages}\"");
        let mut updated = String::with_capacity(opened.text.len() + 32);
        let mut replaced = false;
        for line in opened.text.split_inclusive('\n') {
            if !replaced && line.trim_start().starts_with("pages:") {
                let ending = if line.ends_with("\r\n") {
                    "\r\n"
                } else if line.ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                updated.push_str(&new_pages_line);
                updated.push_str(ending);
                replaced = true;
            } else {
                updated.push_str(line);
            }
        }
        if !replaced {
            return Ok(false);
        }
        self.core
            .documents
            .replace_text(&self.core, note_path, &updated, SYSTEM_CLIENT, Source::Me, None)
            .map_err(|e| format!("{e:?}"))?;
        Ok(true)
    }
}
