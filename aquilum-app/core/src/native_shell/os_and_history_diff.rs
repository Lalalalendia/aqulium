//! Pure-Rust OS Window State Persistence (`window_state.rs` port without Tauri),
//! Tab Drag-and-Drop Reorder Model (`tabReorderModel.ts` port),
//! and Visual Line-by-Line History Diff (`history/rewrite.ts` & Diff viewer).

use crate::documents::merge::line_diff::{line_diff, split_lines};
use crate::files::document::write_file_atomic_impl;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MIN_WIDTH: f64 = 320.0;
const MIN_HEIGHT: f64 = 240.0;
const MAX_WIDTH: f64 = 7_680.0;
const MAX_HEIGHT: f64 = 4_320.0;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct NativeWindowPlacement {
    pub width: f64,
    pub height: f64,
    pub maximized: bool,
}

impl Default for NativeWindowPlacement {
    fn default() -> Self {
        Self {
            width: 1280.0,
            height: 800.0,
            maximized: false,
        }
    }
}

impl NativeWindowPlacement {
    pub fn is_valid(self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && (MIN_WIDTH..=MAX_WIDTH).contains(&self.width)
            && (MIN_HEIGHT..=MAX_HEIGHT).contains(&self.height)
    }
}

pub struct NativeWindowStateManager {
    path: PathBuf,
    state: Mutex<Option<NativeWindowPlacement>>,
}

impl NativeWindowStateManager {
    pub fn new(app_data_dir: &Path) -> Self {
        let path = app_data_dir.join("window-state.json");
        let state = fs::read_to_string(&path)
            .ok()
            .and_then(|json| serde_json::from_str::<NativeWindowPlacement>(&json).ok())
            .filter(|s| s.is_valid());
        Self {
            path,
            state: Mutex::new(state),
        }
    }

    pub fn current(&self) -> NativeWindowPlacement {
        self.state.lock().unwrap().unwrap_or_default()
    }

    pub fn observe(&self, width: f64, height: f64, maximized: bool) {
        let mut guard = self.state.lock().unwrap();
        let next = match (*guard, maximized) {
            (Some(prev), true) => NativeWindowPlacement {
                maximized: true,
                ..prev
            },
            (_, false) => NativeWindowPlacement {
                width,
                height,
                maximized: false,
            },
            (None, true) => NativeWindowPlacement {
                maximized: true,
                ..NativeWindowPlacement::default()
            },
        };
        if next.is_valid() {
            *guard = Some(next);
        }
    }

    pub fn persist(&self) -> Result<(), String> {
        let Some(state) = *self.state.lock().unwrap() else {
            return Ok(());
        };
        let json = serde_json::to_string(&state).map_err(|e| e.to_string())?;
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        write_file_atomic_impl(&self.path, &json, None).map(|_| ()).map_err(|e| e.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TabBox {
    pub left: f32,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReorderLayout {
    pub target: usize,
    pub shift: Vec<f32>,
}

pub fn reorder_layout(boxes: &[TabBox], from: usize, offset: f32) -> ReorderLayout {
    let mut shift = vec![0.0; boxes.len()];
    if from >= boxes.len() {
        return ReorderLayout { target: from, shift };
    }
    let dragged = boxes[from];
    let width = dragged.width;
    let left = dragged.left + offset;
    let right = left + width;
    let mut target = from;

    for (index, b) in boxes.iter().enumerate() {
        if index == from {
            continue;
        }
        let middle = b.left + b.width * 0.5;
        if index < from && left < middle {
            shift[index] = width;
            target = target.min(index);
        } else if index > from && right > middle {
            shift[index] = -width;
            target = target.max(index);
        }
    }
    shift[from] = offset;
    ReorderLayout { target, shift }
}

pub fn resting_offset(boxes: &[TabBox], from: usize, target: usize) -> f32 {
    if from == target || from >= boxes.len() || target >= boxes.len() {
        return 0.0;
    }
    let dragged = boxes[from];
    if target > from {
        let last = boxes[target];
        last.left + last.width - (dragged.left + dragged.width)
    } else {
        boxes[target].left - dragged.left
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDiffKind {
    Unchanged,
    Added,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryDiffLine {
    pub kind: HistoryDiffKind,
    pub text: String,
}

pub fn compute_version_diff(previous_text: &str, current_text: &str) -> Vec<HistoryDiffLine> {
    let old_lines = split_lines(previous_text);
    let new_lines = split_lines(current_text);
    let diff = line_diff(&old_lines, &new_lines);

    let mut out = Vec::new();
    let mut old_idx = 0;
    for hunk in diff.hunks {
        while old_idx < hunk.from {
            out.push(HistoryDiffLine {
                kind: HistoryDiffKind::Unchanged,
                text: old_lines[old_idx].to_owned(),
            });
            old_idx += 1;
        }
        for removed in &old_lines[hunk.from..hunk.to] {
            out.push(HistoryDiffLine {
                kind: HistoryDiffKind::Removed,
                text: (*removed).to_owned(),
            });
        }
        for added in &hunk.lines {
            out.push(HistoryDiffLine {
                kind: HistoryDiffKind::Added,
                text: added.clone(),
            });
        }
        old_idx = hunk.to;
    }
    while old_idx < old_lines.len() {
        out.push(HistoryDiffLine {
            kind: HistoryDiffKind::Unchanged,
            text: old_lines[old_idx].to_owned(),
        });
        old_idx += 1;
    }
    out
}
