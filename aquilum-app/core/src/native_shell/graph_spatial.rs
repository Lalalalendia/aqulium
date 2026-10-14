//! Spatial PickGrid ($O(1)$ node hit-testing), Priority Label Candidate Picker
//! (`LABEL_LIMIT = 140`), and Animated Date Filter (`pickGrid.ts`, `labelCandidates.ts`,
//! `dateFilter.ts`, `nodeMetrics.ts` ports).

use super::graph_view::DecodedGraphNode;
use std::collections::{HashMap, HashSet};

pub const LABEL_LIMIT: usize = 140;
pub const MAX_NODE_PIXELS: f32 = 24.0;
pub const PICK_SLACK_PIXELS: f32 = 6.0;
pub const FADE_SECONDS: f32 = 0.16;

pub fn node_radius_pixels(degree: u32, scale: f32, size_scale: f32, spread: f32) -> f32 {
    let base = 3.2 + (degree as f32).sqrt() * 1.35;
    let scaled = base * size_scale * (scale * spread).powf(0.35);
    scaled.clamp(2.5, MAX_NODE_PIXELS)
}

#[derive(Clone, Debug)]
pub struct PickGrid {
    pub cell: f32,
    pub columns: usize,
    pub rows: usize,
    pub min_x: f32,
    pub min_y: f32,
    pub offsets: Vec<u32>,
    pub nodes: Vec<u32>,
}

impl PickGrid {
    pub fn build(nodes: &[DecodedGraphNode]) -> Option<Self> {
        if nodes.is_empty() {
            return None;
        }
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for n in nodes {
            min_x = min_x.min(n.x);
            max_x = max_x.max(n.x);
            min_y = min_y.min(n.y);
            max_y = max_y.max(n.y);
        }
        let width = (max_x - min_x).max(1e-3);
        let height = (max_y - min_y).max(1e-3);
        let target = ((nodes.len() as f32 / 2.0).sqrt().floor() as usize).max(1);
        let cell = width.max(height) / (target as f32);
        let columns = ((width / cell).ceil() as usize).max(1);
        let rows = ((height / cell).ceil() as usize).max(1);

        let total_cells = columns * rows;
        let mut offsets = vec![0u32; total_cells + 1];
        let mut cell_of = vec![0usize; nodes.len()];

        for (i, n) in nodes.iter().enumerate() {
            let col = (((n.x - min_x) / cell).floor() as isize).clamp(0, columns as isize - 1) as usize;
            let row = (((n.y - min_y) / cell).floor() as isize).clamp(0, rows as isize - 1) as usize;
            let idx = row * columns + col;
            cell_of[i] = idx;
            offsets[idx + 1] += 1;
        }
        for idx in 0..total_cells {
            offsets[idx + 1] += offsets[idx];
        }
        let mut cursor = offsets.clone();
        let mut grid_nodes = vec![0u32; nodes.len()];
        for (i, &c_idx) in cell_of.iter().enumerate() {
            let slot = cursor[c_idx] as usize;
            grid_nodes[slot] = i as u32;
            cursor[c_idx] += 1;
        }

        Some(Self {
            cell,
            columns,
            rows,
            min_x,
            min_y,
            offsets,
            nodes: grid_nodes,
        })
    }

    pub fn visit_area(&self, min_x: f32, min_y: f32, max_x: f32, max_y: f32, mut visit: impl FnMut(u32)) {
        let min_col = (((min_x - self.min_x) / self.cell).floor() as isize).max(0) as usize;
        let max_col = (((max_x - self.min_x) / self.cell).floor() as isize)
            .min(self.columns as isize - 1)
            .max(-1);
        let min_row = (((min_y - self.min_y) / self.cell).floor() as isize).max(0) as usize;
        let max_row = (((max_y - self.min_y) / self.cell).floor() as isize)
            .min(self.rows as isize - 1)
            .max(-1);
        if max_col < 0 || max_row < 0 {
            return;
        }
        for row in min_row..=(max_row as usize) {
            for col in min_col..=(max_col as usize) {
                let cell = row * self.columns + col;
                let start = self.offsets[cell] as usize;
                let end = self.offsets[cell + 1] as usize;
                for &node in &self.nodes[start..end] {
                    visit(node);
                }
            }
        }
    }

    pub fn pick_node(
        &self,
        nodes: &[DecodedGraphNode],
        world_x: f32,
        world_y: f32,
        scale: f32,
        size_scale: f32,
        spread: f32,
    ) -> Option<u32> {
        let pixels_per_cell = (scale * spread).max(1e-4);
        let target_x = world_x / spread;
        let target_y = world_y / spread;
        let reach = (MAX_NODE_PIXELS + PICK_SLACK_PIXELS) / pixels_per_cell;
        let mut best: Option<(u32, f32)> = None;

        self.visit_area(
            target_x - reach,
            target_y - reach,
            target_x + reach,
            target_y + reach,
            |node_idx| {
                let Some(n) = nodes.get(node_idx as usize) else { return };
                let dist = (n.x - target_x).hypot(n.y - target_y);
                let radius = (node_radius_pixels(n.degree, scale, size_scale, spread)
                    + PICK_SLACK_PIXELS)
                    / pixels_per_cell;
                if dist <= radius && best.is_none_or(|(_, b_dist)| dist < b_dist) {
                    best = Some((node_idx, dist));
                }
            },
        );
        best.map(|(idx, _)| idx)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LabelCandidate {
    pub node: u32,
    pub radius: f32,
    pub fade: f32,
    pub eligible: bool,
}

pub fn collect_label_candidates(
    nodes: &[DecodedGraphNode],
    grid: Option<&PickGrid>,
    center_x: f32,
    center_y: f32,
    scale: f32,
    size_scale: f32,
    spread: f32,
    created_from: f32,
    hovered: Option<u32>,
    view_width: f32,
    view_height: f32,
    fades: &HashMap<u32, f32>,
    limit: usize,
) -> Vec<LabelCandidate> {
    let mut order = Vec::new();
    if let Some(g) = grid {
        let pixels_per_unit = (scale * spread).max(1e-4);
        let reach_x = view_width * 0.5 / pixels_per_unit;
        let reach_y = view_height * 0.5 / pixels_per_unit;
        let cx = center_x / spread;
        let cy = center_y / spread;
        g.visit_area(cx - reach_x, cy - reach_y, cx + reach_x, cy + reach_y, |node_idx| {
            if Some(node_idx) == hovered {
                return;
            }
            let Some(n) = nodes.get(node_idx as usize) else { return };
            if n.created_days < created_from {
                return;
            }
            order.push(LabelCandidate {
                node: node_idx,
                radius: node_radius_pixels(n.degree, scale, size_scale, spread),
                fade: fades.get(&node_idx).copied().unwrap_or(0.0),
                eligible: true,
            });
        });
    }

    order.sort_by(|a, b| {
        b.fade
            .total_cmp(&a.fade)
            .then_with(|| b.radius.total_cmp(&a.radius))
            .then_with(|| a.node.cmp(&b.node))
    });
    order.truncate(limit.max(1));

    if let Some(h_idx) = hovered {
        if let Some(n) = nodes.get(h_idx as usize) {
            order.insert(
                0,
                LabelCandidate {
                    node: h_idx,
                    radius: node_radius_pixels(n.degree, scale, size_scale, spread),
                    fade: fades.get(&h_idx).copied().unwrap_or(1.0),
                    eligible: true,
                },
            );
        }
    }

    let taken: HashSet<u32> = order.iter().map(|c| c.node).collect();
    for (&node_idx, &fade) in fades {
        if !taken.contains(&node_idx) {
            if let Some(n) = nodes.get(node_idx as usize) {
                order.push(LabelCandidate {
                    node: node_idx,
                    radius: node_radius_pixels(n.degree, scale, size_scale, spread),
                    fade,
                    eligible: false,
                });
            }
        }
    }
    order
}

#[derive(Clone, Debug, Default)]
pub struct GraphDateFilter {
    pub alpha: Vec<u8>,
    pub targets: Vec<u8>,
    pub visible_count: usize,
    moving: HashSet<usize>,
}

impl GraphDateFilter {
    pub fn adopt(&mut self, nodes: &[DecodedGraphNode], created_from: f32) {
        self.alpha = vec![0; nodes.len()];
        self.targets = vec![0; nodes.len()];
        self.retarget(nodes, created_from);
        self.alpha.copy_from_slice(&self.targets);
        self.moving.clear();
    }

    pub fn retarget(&mut self, nodes: &[DecodedGraphNode], created_from: f32) {
        let mut visible = 0;
        for (i, n) in nodes.iter().enumerate() {
            let shown = !(n.created_days < created_from);
            if shown {
                visible += 1;
            }
            let target = if shown { 255 } else { 0 };
            self.targets[i] = target;
            if self.alpha[i] == target {
                self.moving.remove(&i);
            } else {
                self.moving.insert(i);
            }
        }
        self.visible_count = visible;
    }

    pub fn advance(&mut self, seconds: f32) -> bool {
        let step = 1.0 - (-seconds / FADE_SECONDS.max(1e-4)).exp();
        let indices: Vec<usize> = self.moving.iter().copied().collect();
        for idx in indices {
            let goal = i32::from(self.targets[idx]);
            let from = i32::from(self.alpha[idx]);
            let gap = goal - from;
            let next = if gap.abs() <= 2 {
                goal
            } else {
                from + ((gap as f32) * step).round() as i32
            };
            let clamped = next.clamp(0, 255) as u8;
            self.alpha[idx] = clamped;
            if clamped == self.targets[idx] {
                self.moving.remove(&idx);
            }
        }
        !self.moving.is_empty()
    }
}
