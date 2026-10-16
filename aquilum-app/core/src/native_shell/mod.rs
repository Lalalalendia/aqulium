//! Aquilum Native Shell (Strangler Fig Stages 1, 5, 7).
//!
//! Provides the pure-Rust workspace shell, multi-tab manager with Yrs sticky cursor
//! persistence, back/forward navigation history, file tree explorer, global search
//! & quick switcher, backlinks/semantic analysis panel, note version history & diff
//! inspector, native 2D knowledge graph renderer, settings/templates controller,
//! and Stage 7 cutover verification gate.

pub mod cutover;
pub mod export;
pub mod shortcuts_theme_i18n;
pub mod trash_view;
pub mod graph_spatial;
pub mod graph_view;
pub mod os_and_history_diff;
pub mod settings_view;
pub mod shell;
pub mod sidebar;
pub mod tabs;

pub use cutover::{verify_strangler_fig_cutover, CutoverReport, StageCheck};
pub use export::NativeExporter;
pub use shortcuts_theme_i18n::{
    match_shortcut, parse_hex_rgb, t, Locale, ShortcutId, ShortcutToken, ThemePalette,
    UiScaleState, SHORTCUTS,
};
pub use trash_view::{NativeTrashController, RetentionCleanupSummary, WikixivInspector};
pub use graph_spatial::{
    collect_label_candidates, node_radius_pixels, GraphDateFilter, LabelCandidate, PickGrid,
    LABEL_LIMIT,
};
pub use graph_view::{DecodedGraphEdge, DecodedGraphNode, GraphColorMode, NativeGraphView};
pub use os_and_history_diff::{
    compute_version_diff, reorder_layout, resting_offset, HistoryDiffKind, HistoryDiffLine,
    NativeWindowPlacement, NativeWindowStateManager, ReorderLayout, TabBox,
};
pub use settings_view::{NativeSettingsController, NativeTemplateService};
pub use shell::NativeWorkspaceShell;
pub use sidebar::{
    BacklinksInspector, BacklinksSnapshot, GlobalSearchController, HistoryInspector,
    WorkspaceExplorer,
};
pub use tabs::{NativeTab, NativeTabManager, NavigationHistory};

#[cfg(test)]
mod tests;
