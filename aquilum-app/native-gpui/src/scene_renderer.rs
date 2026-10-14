//! Pure-Rust GPUI Scene Graph & Primitive Layout Engine (`GpuiSceneGraph`).

use crate::workspace_window::{
    GpuiActiveCenterSurface, GpuiModalOverlay, GpuiSidebarMode, GpuiWindowLayoutFrame,
};
use aquilum_core::native_editor::MarkdownTokenKind;
use aquilum_core::native_shell::{ThemePalette, UiScaleState};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectPx {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl RectPx {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneQuad {
    pub bounds: RectPx,
    pub bg_rgb: (u8, u8, u8),
    pub border_rgb: Option<(u8, u8, u8)>,
    pub corner_radius_px: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneTextRun {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub font_size_px: f32,
    pub color_rgb: (u8, u8, u8),
    pub bold: bool,
    pub monospace: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneLine {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke_width_px: f32,
    pub color_rgb: (u8, u8, u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitboxTarget {
    TabSelect(Uuid),
    TabClose(Uuid),
    NavBack,
    NavForward,
    OpenGraphTab,
    SidebarItem(String),
    EditorSurface,
    GraphNode(u32),
    ModalDismiss,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SceneHitbox {
    pub bounds: RectPx,
    pub target: HitboxTarget,
}

#[derive(Clone, Debug, Default)]
pub struct GpuiSceneGraph {
    pub quads: Vec<SceneQuad>,
    pub lines: Vec<SceneLine>,
    pub text_runs: Vec<SceneTextRun>,
    pub hitboxes: Vec<SceneHitbox>,
}

impl GpuiSceneGraph {
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&HitboxTarget> {
        self.hitboxes
            .iter()
            .rev()
            .find(|h| h.bounds.contains(x, y))
            .map(|h| &h.target)
    }
}

pub fn build_window_scene(
    frame: &GpuiWindowLayoutFrame,
    palette: ThemePalette,
    scale_state: UiScaleState,
    window_width: f32,
    window_height: f32,
) -> GpuiSceneGraph {
    let mut scene = GpuiSceneGraph::default();
    let zoom = scale_state.scale.clamp(0.5, 2.0);
    let titlebar_h = 38.0 * zoom;
    let sidebar_w = if scale_state.focus_mode || frame.sidebar_mode == GpuiSidebarMode::Hidden {
        0.0
    } else {
        240.0 * zoom
    };
    let inspector_w = if scale_state.focus_mode
        || frame.inspector_mode == crate::workspace_window::GpuiInspectorMode::Hidden
    {
        0.0
    } else {
        260.0 * zoom
    };

    scene.quads.push(SceneQuad {
        bounds: RectPx {
            x: 0.0,
            y: 0.0,
            width: window_width,
            height: window_height,
        },
        bg_rgb: palette.bg_primary,
        border_rgb: None,
        corner_radius_px: 0.0,
    });

    scene.quads.push(SceneQuad {
        bounds: RectPx {
            x: 0.0,
            y: 0.0,
            width: window_width,
            height: titlebar_h,
        },
        bg_rgb: palette.bg_secondary,
        border_rgb: Some(palette.border_subtle),
        corner_radius_px: 0.0,
    });

    let mut tab_x = 88.0 * zoom;
    let tab_w = 160.0 * zoom;
    for tab in &frame.tabs {
        let is_active = frame.active_tab_id == Some(tab.tab_id);
        let tab_rect = RectPx {
            x: tab_x,
            y: 4.0 * zoom,
            width: tab_w,
            height: titlebar_h - 4.0 * zoom,
        };
        scene.quads.push(SceneQuad {
            bounds: tab_rect,
            bg_rgb: if is_active {
                palette.bg_primary
            } else {
                palette.bg_secondary
            },
            border_rgb: Some(palette.border_subtle),
            corner_radius_px: 6.0,
        });
        scene.text_runs.push(SceneTextRun {
            x: tab_x + 12.0 * zoom,
            y: 11.0 * zoom,
            text: tab.title.clone(),
            font_size_px: 13.0 * zoom,
            color_rgb: if is_active {
                palette.text_primary
            } else {
                palette.text_muted
            },
            bold: is_active,
            monospace: false,
        });
        scene.hitboxes.push(SceneHitbox {
            bounds: tab_rect,
            target: HitboxTarget::TabSelect(tab.tab_id),
        });
        tab_x += tab_w + 4.0 * zoom;
    }

    if sidebar_w > 0.0 {
        scene.quads.push(SceneQuad {
            bounds: RectPx {
                x: 0.0,
                y: titlebar_h,
                width: sidebar_w,
                height: window_height - titlebar_h,
            },
            bg_rgb: palette.bg_secondary,
            border_rgb: Some(palette.border_subtle),
            corner_radius_px: 0.0,
        });
        let mut row_y = titlebar_h + 12.0 * zoom;
        for item in &frame.root_files {
            let item_rect = RectPx {
                x: 8.0 * zoom,
                y: row_y,
                width: sidebar_w - 16.0 * zoom,
                height: 26.0 * zoom,
            };
            scene.text_runs.push(SceneTextRun {
                x: 16.0 * zoom,
                y: row_y + 5.0 * zoom,
                text: item.name.clone(),
                font_size_px: 13.0 * zoom,
                color_rgb: palette.text_primary,
                bold: false,
                monospace: false,
            });
            scene.hitboxes.push(SceneHitbox {
                bounds: item_rect,
                target: HitboxTarget::SidebarItem(item.id.clone()),
            });
            row_y += 28.0 * zoom;
        }
    }

    let center_rect = RectPx {
        x: sidebar_w,
        y: titlebar_h,
        width: (window_width - sidebar_w - inspector_w).max(100.0),
        height: (window_height - titlebar_h).max(100.0),
    };
    scene.hitboxes.push(SceneHitbox {
        bounds: center_rect,
        target: HitboxTarget::EditorSurface,
    });

    match &frame.center {
        GpuiActiveCenterSurface::Editor { frame: ed_frame, .. } => {
            let line_h = 24.0 * zoom;
            let text_x = center_rect.x + 48.0 * zoom;
            let mut text_y = center_rect.y + 20.0 * zoom;
            let max_visible_lines = ((center_rect.height / line_h) as usize).max(1);
            for (idx, line) in ed_frame.viewport.text.lines().take(max_visible_lines).enumerate() {
                let doc_line_num = ed_frame.viewport.start_line + idx + 1;
                scene.text_runs.push(SceneTextRun {
                    x: center_rect.x + 12.0 * zoom,
                    y: text_y,
                    text: doc_line_num.to_string(),
                    font_size_px: 12.0 * zoom,
                    color_rgb: palette.text_muted,
                    bold: false,
                    monospace: true,
                });
                let is_heading = ed_frame
                    .viewport
                    .markdown_spans
                    .iter()
                    .any(|s| matches!(s.kind, MarkdownTokenKind::Heading(_)));
                scene.text_runs.push(SceneTextRun {
                    x: text_x,
                    y: text_y,
                    text: line.to_owned(),
                    font_size_px: 15.0 * zoom,
                    color_rgb: palette.text_primary,
                    bold: is_heading && line.starts_with('#'),
                    monospace: false,
                });
                text_y += line_h;
            }
        }
        GpuiActiveCenterSurface::Graph { nodes, edges, .. } => {
            for edge in edges {
                if let (Some((_, (x1, y1))), Some((_, (x2, y2)))) =
                    (nodes.get(edge.source as usize), nodes.get(edge.target as usize))
                {
                    scene.lines.push(SceneLine {
                        x1: center_rect.x + *x1,
                        y1: center_rect.y + *y1,
                        x2: center_rect.x + *x2,
                        y2: center_rect.y + *y2,
                        stroke_width_px: 1.0,
                        color_rgb: palette.border_subtle,
                    });
                }
            }
            for (node, (sx, sy)) in nodes {
                let r = 6.0 * zoom;
                let node_rect = RectPx {
                    x: center_rect.x + *sx - r,
                    y: center_rect.y + *sy - r,
                    width: r * 2.0,
                    height: r * 2.0,
                };
                scene.quads.push(SceneQuad {
                    bounds: node_rect,
                    bg_rgb: node.color_rgb,
                    border_rgb: None,
                    corner_radius_px: r,
                });
                scene.hitboxes.push(SceneHitbox {
                    bounds: node_rect,
                    target: HitboxTarget::GraphNode(node.index),
                });
            }
        }
        GpuiActiveCenterSurface::Reader {
            book_title,
            chapter_title,
            visible_paragraphs,
            ..
        } => {
            let mut py = center_rect.y + 24.0 * zoom;
            scene.text_runs.push(SceneTextRun {
                x: center_rect.x + 48.0 * zoom,
                y: py,
                text: format!("{book_title} — {chapter_title}"),
                font_size_px: 18.0 * zoom,
                color_rgb: palette.text_primary,
                bold: true,
                monospace: false,
            });
            py += 32.0 * zoom;
            for para in visible_paragraphs {
                scene.text_runs.push(SceneTextRun {
                    x: center_rect.x + 48.0 * zoom,
                    y: py,
                    text: para.clone(),
                    font_size_px: 16.0 * zoom,
                    color_rgb: palette.text_primary,
                    bold: false,
                    monospace: false,
                });
                py += 28.0 * zoom;
            }
        }
        GpuiActiveCenterSurface::EmptyState => {}
    }

    if frame.modal != GpuiModalOverlay::None {
        let modal_rect = RectPx {
            x: window_width * 0.25,
            y: window_height * 0.15,
            width: window_width * 0.5,
            height: window_height * 0.6,
        };
        scene.quads.push(SceneQuad {
            bounds: modal_rect,
            bg_rgb: palette.bg_elevated,
            border_rgb: Some(palette.accent_rgb),
            corner_radius_px: 12.0,
        });
        scene.hitboxes.push(SceneHitbox {
            bounds: modal_rect,
            target: HitboxTarget::ModalDismiss,
        });
    }

    scene
}
