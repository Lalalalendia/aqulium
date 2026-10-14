//! Stage 6 of Aquilum Native (Strangler Fig Migration):
//! Experimental native FB2 reader and XHTML parser (EPUB ZIP not yet supported), with CFI tracking
//! persistence, and bidirectional `> [!quote]` + `aquilum-reader:` quote links.

pub mod library;
pub use library::{BookLibraryController, BookLibraryEntry, CoverPatternKind};
use crate::app_core::Core;
use crate::ui_state::models::SaveReaderStateInput;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookFormat {
    Fb2,
    Epub,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BookMetadata {
    pub title: String,
    pub authors: Vec<String>,
    pub language: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookParagraph {
    pub index: usize,
    pub text: String,
    pub is_heading: bool,
    pub cfi: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookChapter {
    pub index: usize,
    pub title: String,
    pub paragraphs: Vec<BookParagraph>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBookDocument {
    pub path: PathBuf,
    pub relative_path: String,
    pub format: BookFormat,
    pub metadata: BookMetadata,
    pub chapters: Vec<BookChapter>,
    pub total_paragraphs: usize,
}

pub struct NativeBookReader {
    core: Arc<Core>,
    workspace_id: Uuid,
    book: NativeBookDocument,
    current_chapter: usize,
    current_paragraph: usize,
}

impl NativeBookReader {
    pub fn open(
        core: Arc<Core>,
        workspace_id: Uuid,
        workspace_root: &Path,
        book_path: &Path,
    ) -> Result<Self, String> {
        let relative_path = book_path
            .strip_prefix(workspace_root)
            .unwrap_or(book_path)
            .to_string_lossy()
            .replace('\\', "/");

        let ext = book_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();

        if ext == "epub" {
            return Err("Native EPUB ZIP parsing is not implemented yet; use the existing book renderer".to_owned());
        }
        let raw = fs::read_to_string(book_path).map_err(|e| format!("failed to read book: {e}"))?;
        let book = if ext == "fb2" || raw.contains("<FictionBook") {
            parse_fb2_document(book_path, &relative_path, &raw)
        } else if ext == "epub" || raw.contains("<html") || raw.contains("<body") {
            parse_xhtml_epub_document(book_path, &relative_path, &raw)
        } else {
            return Err(format!("unsupported book format: {ext}"));
        };

        let mut reader = Self {
            core,
            workspace_id,
            book,
            current_chapter: 0,
            current_paragraph: 0,
        };

        // Restore saved CFI / progress from UiStateService if available
        if let Ok(Some(saved)) = reader
            .core
            .ui_state
            .load_reader_state(workspace_id, &relative_path)
        {
            if let Some(cfi) = saved.cfi.as_deref() {
                reader.jump_to_cfi(cfi);
            }
        }

        Ok(reader)
    }

    pub fn book(&self) -> &NativeBookDocument {
        &self.book
    }

    pub fn current_position(&self) -> (usize, usize) {
        (self.current_chapter, self.current_paragraph)
    }

    pub fn current_cfi(&self) -> String {
        self.book
            .chapters
            .get(self.current_chapter)
            .and_then(|ch| ch.paragraphs.get(self.current_paragraph))
            .map(|p| p.cfi.clone())
            .unwrap_or_else(|| make_cfi(self.current_chapter, self.current_paragraph))
    }

    pub fn progress_percent(&self) -> i64 {
        if self.book.total_paragraphs <= 1 {
            return 100;
        }
        let mut seen = 0usize;
        for ch in self.book.chapters.iter().take(self.current_chapter) {
            seen += ch.paragraphs.len();
        }
        seen += self.current_paragraph;
        ((seen * 100) / self.book.total_paragraphs.max(1)).min(100) as i64
    }

    pub fn navigate_to(&mut self, chapter_idx: usize, paragraph_idx: usize, now_ms: i64) -> Result<(), String> {
        if self.book.chapters.is_empty() {
            return Ok(());
        }
        self.current_chapter = chapter_idx.min(self.book.chapters.len() - 1);
        let max_para = self.book.chapters[self.current_chapter]
            .paragraphs
            .len()
            .saturating_sub(1);
        self.current_paragraph = paragraph_idx.min(max_para);
        self.save_progress(now_ms)
    }

    pub fn jump_to_cfi(&mut self, cfi: &str) -> bool {
        for (c_idx, ch) in self.book.chapters.iter().enumerate() {
            for (p_idx, p) in ch.paragraphs.iter().enumerate() {
                if p.cfi == cfi {
                    self.current_chapter = c_idx;
                    self.current_paragraph = p_idx;
                    return true;
                }
            }
        }
        if let Some((c_idx, p_idx)) = parse_cfi_indices(cfi) {
            if c_idx < self.book.chapters.len() {
                self.current_chapter = c_idx;
                let max_p = self.book.chapters[c_idx].paragraphs.len().saturating_sub(1);
                self.current_paragraph = p_idx.min(max_p);
                return true;
            }
        }
        false
    }

    pub fn save_progress(&self, now_ms: i64) -> Result<(), String> {
        let input = SaveReaderStateInput {
            workspace_id: self.workspace_id,
            book_file: self.book.relative_path.clone(),
            current: self.progress_percent(),
            cfi: Some(self.current_cfi()),
            now_ms,
        };
        self.core
            .ui_state
            .save_reader_state(&input)
            .map_err(|e| format!("failed to save reader state: {e:?}"))
    }

    /// Formats a linked Markdown quote block (`> [!quote]`) with an `aquilum-reader://`
    /// deep link pointing to the exact chapter/paragraph CFI.
    pub fn format_linked_quote(&self, selected_text: &str) -> String {
        let cfi = self.current_cfi();
        let author_suffix = if self.book.metadata.authors.is_empty() {
            String::new()
        } else {
            format!(" — {}", self.book.metadata.authors.join(", "))
        };
        let header = format!("> [!quote] {}{}", self.book.metadata.title, author_suffix);
        let mut quoted_lines = String::new();
        for line in selected_text.lines() {
            quoted_lines.push_str("> ");
            quoted_lines.push_str(line.trim());
            quoted_lines.push('\n');
        }
        let url = format!("aquilum-reader://{}#cfi={}", self.book.relative_path, cfi);
        format!("{header}\n{quoted_lines}> [Открыть в книге]({url})\n")
    }
}

pub fn make_cfi(chapter_idx: usize, para_idx: usize) -> String {
    format!("/6/{}!/4/{}:0", (chapter_idx + 1) * 2, (para_idx + 1) * 2)
}

pub fn parse_cfi_indices(cfi: &str) -> Option<(usize, usize)> {
    let after_six = cfi.strip_prefix("/6/")?;
    let (chap_str, rest) = after_six.split_once("!/4/")?;
    let (para_str, _) = rest.split_once(':').unwrap_or((rest, "0"));
    let chap_step: usize = chap_str.parse().ok()?;
    let para_step: usize = para_str.parse().ok()?;
    Some((chap_step.saturating_sub(2) / 2, para_step.saturating_sub(2) / 2))
}

pub fn parse_reader_quote_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("aquilum-reader://")?;
    let (book_path, cfi) = rest.split_once("#cfi=")?;
    Some((book_path.to_owned(), cfi.to_owned()))
}

fn extract_tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start_pos = xml.find(&open)?;
    let after_open_bracket = xml[start_pos..].find('>')? + start_pos + 1;
    let end_pos = xml[after_open_bracket..].find(&close)? + after_open_bracket;
    Some(strip_xml_tags(&xml[after_open_bracket..end_pos]).trim().to_owned())
}

fn strip_xml_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

fn parse_fb2_document(path: &Path, relative_path: &str, xml: &str) -> NativeBookDocument {
    let title = extract_tag_text(xml, "book-title").unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Book".to_owned())
    });
    let first_name = extract_tag_text(xml, "first-name").unwrap_or_default();
    let last_name = extract_tag_text(xml, "last-name").unwrap_or_default();
    let author = format!("{first_name} {last_name}").trim().to_owned();
    let authors = if author.is_empty() { Vec::new() } else { vec![author] };
    let language = extract_tag_text(xml, "lang");

    let mut chapters = Vec::new();
    let mut total_paragraphs = 0;

    let body_slice = if let Some(b_start) = xml.find("<body") {
        let b_end = xml[b_start..].find("</body>").map_or(xml.len(), |i| b_start + i);
        &xml[b_start..b_end]
    } else {
        xml
    };

    for (sec_idx, sec_chunk) in body_slice.split("<section").skip(1).enumerate() {
        let sec_end = sec_chunk.find("</section>").unwrap_or(sec_chunk.len());
        let sec_body = &sec_chunk[..sec_end];
        let chap_title = extract_tag_text(sec_body, "title")
            .unwrap_or_else(|| format!("Глава {}", sec_idx + 1));

        let mut paragraphs = Vec::new();
        for p_chunk in sec_body.split("<p") {
            let Some(gt) = p_chunk.find('>') else { continue };
            let Some(end_p) = p_chunk[gt + 1..].find("</p>") else { continue };
            let text = strip_xml_tags(&p_chunk[gt + 1..gt + 1 + end_p]).trim().to_owned();
            if !text.is_empty() {
                let p_idx = paragraphs.len();
                paragraphs.push(BookParagraph {
                    index: p_idx,
                    text,
                    is_heading: false,
                    cfi: make_cfi(sec_idx, p_idx),
                });
                total_paragraphs += 1;
            }
        }
        if !paragraphs.is_empty() {
            chapters.push(BookChapter {
                index: chapters.len(),
                title: chap_title,
                paragraphs,
            });
        }
    }

    if chapters.is_empty() {
        let text = strip_xml_tags(body_slice).trim().to_owned();
        let paragraphs = vec![BookParagraph {
            index: 0,
            text,
            is_heading: false,
            cfi: make_cfi(0, 0),
        }];
        total_paragraphs = 1;
        chapters.push(BookChapter {
            index: 0,
            title: title.clone(),
            paragraphs,
        });
    }

    NativeBookDocument {
        path: path.to_path_buf(),
        relative_path: relative_path.to_owned(),
        format: BookFormat::Fb2,
        metadata: BookMetadata {
            title,
            authors,
            language,
        },
        chapters,
        total_paragraphs,
    }
}

fn parse_xhtml_epub_document(path: &Path, relative_path: &str, html: &str) -> NativeBookDocument {
    let title = extract_tag_text(html, "title").unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "EPUB".to_owned())
    });
    let mut paragraphs = Vec::new();
    for p_chunk in html.split("<p") {
        let Some(gt) = p_chunk.find('>') else { continue };
        let Some(end_p) = p_chunk[gt + 1..].find("</p>") else { continue };
        let text = strip_xml_tags(&p_chunk[gt + 1..gt + 1 + end_p]).trim().to_owned();
        if !text.is_empty() {
            let p_idx = paragraphs.len();
            paragraphs.push(BookParagraph {
                index: p_idx,
                text,
                is_heading: false,
                cfi: make_cfi(0, p_idx),
            });
        }
    }
    let total_paragraphs = paragraphs.len();
    NativeBookDocument {
        path: path.to_path_buf(),
        relative_path: relative_path.to_owned(),
        format: BookFormat::Epub,
        metadata: BookMetadata {
            title: title.clone(),
            authors: Vec::new(),
            language: None,
        },
        chapters: vec![BookChapter {
            index: 0,
            title,
            paragraphs,
        }],
        total_paragraphs,
    }
}
