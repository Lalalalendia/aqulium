use crate::app_core::Core;
use crate::search::error::SearchError;
use crate::ui_state::models::GraphCameraState;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphColorMode {
    Created,
    Modified,
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedGraphNode {
    pub index: u32,
    pub path: String,
    pub title: String,
    pub x: f32,
    pub y: f32,
    pub created_days: f32,
    pub modified_days: f32,
    pub degree: u32,
    pub color_rgb: (u8, u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodedGraphEdge {
    pub source: u32,
    pub target: u32,
}

/// Native 2D Knowledge Graph view that decodes `SearchService::render_graph` binary
/// snapshots directly in Rust and handles camera transforms, color modes, hit-testing,
/// and neighbor highlighting.
pub struct NativeGraphView {
    core: Arc<Core>,
    epoch: u64,
    nodes: Vec<DecodedGraphNode>,
    edges: Vec<DecodedGraphEdge>,
    pub color_mode: GraphColorMode,
    pub camera: GraphCameraState,
    pub focused_node: Option<u32>,
}

impl NativeGraphView {
    pub fn new(core: Arc<Core>) -> Self {
        Self {
            core,
            epoch: 0,
            nodes: Vec::new(),
            edges: Vec::new(),
            color_mode: GraphColorMode::Modified,
            camera: GraphCameraState {
                center_x: 0.0,
                center_y: 0.0,
                scale: 1.0,
            },
            focused_node: None,
        }
    }

    pub fn refresh(&mut self, workspace_root: &Path) -> Result<(), SearchError> {
        let ws = workspace_root.to_string_lossy();
        let _ = self.core.search.prepare(&ws);
        for _ in 0..200 {
            let status = self.core.search.status(Some(&ws));
            if !status.updating {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let bytes = self.core.search.render_graph(&ws)?;
        self.decode_snapshot(&bytes)?;
        self.recompute_colors();
        Ok(())
    }

    pub fn nodes(&self) -> &[DecodedGraphNode] {
        &self.nodes
    }

    pub fn edges(&self) -> &[DecodedGraphEdge] {
        &self.edges
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn set_color_mode(&mut self, mode: GraphColorMode) {
        self.color_mode = mode;
        self.recompute_colors();
    }

    pub fn pan(&mut self, dx_world: f64, dy_world: f64) {
        self.camera.center_x += dx_world;
        self.camera.center_y += dy_world;
    }

    pub fn zoom(&mut self, factor: f64) {
        self.camera.scale = (self.camera.scale * factor).clamp(0.05, 20.0);
    }

    pub fn world_to_screen(&self, wx: f32, wy: f32, viewport_w: f32, viewport_h: f32) -> (f32, f32) {
        let sx = (wx as f64 - self.camera.center_x) * self.camera.scale + (viewport_w as f64 * 0.5);
        let sy = (wy as f64 - self.camera.center_y) * self.camera.scale + (viewport_h as f64 * 0.5);
        (sx as f32, sy as f32)
    }

    pub fn hit_test_node(&self, screen_x: f32, screen_y: f32, viewport_w: f32, viewport_h: f32, radius_px: f32) -> Option<u32> {
        let r2 = radius_px * radius_px;
        let mut best: Option<(u32, f32)> = None;
        for node in &self.nodes {
            let (sx, sy) = self.world_to_screen(node.x, node.y, viewport_w, viewport_h);
            let d2 = (sx - screen_x).powi(2) + (sy - screen_y).powi(2);
            if d2 <= r2 && best.is_none_or(|(_, b_d2)| d2 < b_d2) {
                best = Some((node.index, d2));
            }
        }
        best.map(|(idx, _)| idx)
    }

    pub fn neighbors_of(&self, node_idx: u32) -> Vec<u32> {
        let mut out = Vec::new();
        for edge in &self.edges {
            if edge.source == node_idx {
                out.push(edge.target);
            } else if edge.target == node_idx {
                out.push(edge.source);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn decode_snapshot(&mut self, bytes: &[u8]) -> Result<(), SearchError> {
        if bytes.len() < 16 {
            self.nodes.clear();
            self.edges.clear();
            return Ok(());
        }
        let n = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let e = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let low = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as u64;
        let high = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as u64;
        self.epoch = low | (high << 32);

        let mut offset = 16;
        let read_f32 = |buf: &[u8], off: &mut usize| -> f32 {
            let val = f32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
            *off += 4;
            val
        };
        let read_u32 = |buf: &[u8], off: &mut usize| -> u32 {
            let val = u32::from_le_bytes(buf[*off..*off + 4].try_into().unwrap());
            *off += 4;
            val
        };

        let mut positions = Vec::with_capacity(n * 2);
        for _ in 0..n * 2 {
            positions.push(read_f32(bytes, &mut offset));
        }
        let mut created_days = Vec::with_capacity(n);
        for _ in 0..n {
            created_days.push(read_f32(bytes, &mut offset));
        }
        let mut modified_days = Vec::with_capacity(n);
        for _ in 0..n {
            modified_days.push(read_f32(bytes, &mut offset));
        }
        let mut degrees = Vec::with_capacity(n);
        for _ in 0..n {
            degrees.push(read_u32(bytes, &mut offset));
        }
        let mut edges = Vec::with_capacity(e);
        for _ in 0..e {
            let source = read_u32(bytes, &mut offset);
            let target = read_u32(bytes, &mut offset);
            edges.push(DecodedGraphEdge { source, target });
        }

        // Resolve paths in chunks of 512
        let mut all_paths = vec![String::new(); n];
        for chunk_start in (0..n).step_by(512) {
            let chunk_end = (chunk_start + 512).min(n);
            let indices: Vec<u32> = (chunk_start as u32..chunk_end as u32).collect();
            if let Ok(resolved) = self.core.search.graph_paths(self.epoch, &indices) {
                for (i, p) in resolved.into_iter().enumerate() {
                    all_paths[chunk_start + i] = p;
                }
            }
        }

        self.nodes = (0..n)
            .map(|i| {
                let path = all_paths[i].clone();
                let title = Path::new(&path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("Node {i}"));
                DecodedGraphNode {
                    index: i as u32,
                    path,
                    title,
                    x: positions[i * 2],
                    y: positions[i * 2 + 1],
                    created_days: created_days[i],
                    modified_days: modified_days[i],
                    degree: degrees[i],
                    color_rgb: (140, 155, 175),
                }
            })
            .collect();
        self.edges = edges;
        Ok(())
    }

    fn recompute_colors(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        match self.color_mode {
            GraphColorMode::None => {
                for node in &mut self.nodes {
                    node.color_rgb = (130, 145, 165);
                }
            }
            GraphColorMode::Created | GraphColorMode::Modified => {
                let values: Vec<f32> = self
                    .nodes
                    .iter()
                    .map(|n| match self.color_mode {
                        GraphColorMode::Created => n.created_days,
                        _ => n.modified_days,
                    })
                    .filter(|v| v.is_finite())
                    .collect();
                let min_v = values.iter().copied().fold(f32::INFINITY, f32::min);
                let max_v = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let span = (max_v - min_v).max(1e-5);
                for node in &mut self.nodes {
                    let v = match self.color_mode {
                        GraphColorMode::Created => node.created_days,
                        _ => node.modified_days,
                    };
                    if v.is_finite() {
                        let t = ((v - min_v) / span).clamp(0.0, 1.0);
                        let r = (40.0 + t * 180.0) as u8;
                        let g = (113.0 + (1.0 - (t - 0.5).abs() * 2.0) * 90.0) as u8;
                        let b = (235.0 - t * 150.0) as u8;
                        node.color_rgb = (r, g, b);
                    } else {
                        node.color_rgb = (120, 120, 120);
                    }
                }
            }
        }
    }
}
