//! Native Rust text editor engine (GPUI + Rope + Bounded Viewport) for Aquilum's
//! Strangler Fig migration (Stages 0–3).
//!
//! Designed to replace the WebView2/CodeMirror text editing pipeline while sharing
//! `aquilum-core`'s `DocumentHub`, `DocumentSession`, Yrs CRDT `Replica`, SQLite
//! persistence, and conflict resolution.

pub mod editor;
pub mod frontmatter_and_media;
pub mod history;
pub mod ime;
pub mod markdown_interactive;
pub mod page_search;
pub mod rope;
pub mod selection;
pub mod viewport;

pub use editor::{EditorError, EditorMetrics, KeyCommand, NativeEditorSession};
pub use frontmatter_and_media::{
    format_image_embed, parse_frontmatter, parse_image_embed, FrontmatterValue, ImageAlign,
    ImageCrop, ImageEmbed, MediaKind, ParsedFrontmatter, ResolvedBookFields,
};
pub use history::{EditKind, UndoEntry, UndoStack, COALESCE_WINDOW_MS};
pub use ime::ImeState;
pub use markdown_interactive::{
    parse_table_at_line, toggle_dataview_task, ColumnAlignment, EvaluatedDataviewBlock,
    InteractiveLinkTarget, MarkdownTable,
};
pub use page_search::{PageSearchState, WikiAutocompletePopup};
pub use rope::{ChunkMetrics, LineCol, RopeBuffer};
pub use selection::Selection;
pub use viewport::{
    compute_viewport, scan_viewport_markdown, MarkdownSpan, MarkdownTokenKind, ViewportIme,
    ViewportSelection, ViewportWindow, DEFAULT_CONTEXT_BEFORE_CHARS, MAX_VIEWPORT_CHARS,
};

#[cfg(test)]
mod tests;
