//! Experimental headless native Rust workspace, Rope editor, and render scene model.
//! No physical GPUI window is created by this crate yet.

pub mod editor_view;
pub mod scene_renderer;
pub mod workspace_window;

pub use aquilum_core::native_editor::*;
pub use aquilum_core::native_reader::*;
pub use aquilum_core::native_shell::*;
pub use editor_view::{GpuiEditorAction, GpuiFrameState, GpuiImeBridge, GpuiRopeEditorView};
pub use workspace_window::{
    GpuiActiveCenterSurface, GpuiInspectorMode, GpuiModalOverlay, GpuiSidebarMode,
    GpuiWindowAction, GpuiWindowLayoutFrame, GpuiWorkspaceWindow,
};

pub use scene_renderer::{
    build_window_scene, GpuiSceneGraph, HitboxTarget, RectPx, SceneHitbox, SceneLine, SceneQuad,
    SceneTextRun,
};
