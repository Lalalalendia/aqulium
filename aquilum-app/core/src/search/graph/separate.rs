use std::f64::consts::PI;

const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
const RELAXATION: f64 = 0.55;

pub fn separate(coordinates: &mut [f64], gap: f64, passes: usize) {
    let count = coordinates.len() / 2;
    if count < 2 || gap <= 0.0 {
        return;
    }
    let mut push = vec![0.0f64; coordinates.len()];
    let mut grid = Grid::new();
    for _ in 0..passes {
        grid.rebuild(coordinates, gap);
        push.fill(0.0);
        let mut crowded = false;
        for node in 0..count {
            if grid.resolve(node, coordinates, gap, &mut push) {
                crowded = true;
            }
        }
        if !crowded {
            return;
        }
        for slot in 0..coordinates.len() {
            coordinates[slot] += push[slot] * RELAXATION;
        }
    }
}

#[derive(Default)]
struct Grid {
    columns: i64,
    rows: i64,
    min_x: f64,
    min_y: f64,
    cell: f64,
    offsets: Vec<u32>,
    buckets: Vec<u32>,
    cell_of: Vec<u32>,
    cursor: Vec<u32>,
}

impl Grid {
    fn new() -> Self {
        Self::default()
    }

    fn rebuild(&mut self, coordinates: &[f64], cell: f64) {
        let count = coordinates.len() / 2;
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for node in 0..count {
            min_x = min_x.min(coordinates[node * 2]);
            max_x = max_x.max(coordinates[node * 2]);
            min_y = min_y.min(coordinates[node * 2 + 1]);
            max_y = max_y.max(coordinates[node * 2 + 1]);
        }
        self.columns = (((max_x - min_x) / cell).floor() as i64 + 1).max(1);
        self.rows = (((max_y - min_y) / cell).floor() as i64 + 1).max(1);
        self.min_x = min_x;
        self.min_y = min_y;
        self.cell = cell;
        let cells = (self.columns * self.rows) as usize;

        self.offsets.clear();
        self.offsets.resize(cells + 1, 0);
        self.cell_of.clear();
        self.cell_of.resize(count, 0);
        for node in 0..count {
            let column = index_of(coordinates[node * 2], min_x, cell, self.columns);
            let row = index_of(coordinates[node * 2 + 1], min_y, cell, self.rows);
            let slot = (row * self.columns + column) as u32;
            self.cell_of[node] = slot;
            self.offsets[slot as usize + 1] += 1;
        }
        for slot in 0..cells {
            self.offsets[slot + 1] += self.offsets[slot];
        }
        self.buckets.clear();
        self.buckets.resize(count, 0);
        self.cursor.clear();
        self.cursor.extend_from_slice(&self.offsets);
        for node in 0..count {
            let slot = self.cell_of[node] as usize;
            self.buckets[self.cursor[slot] as usize] = node as u32;
            self.cursor[slot] += 1;
        }
    }

    fn resolve(&self, node: usize, coordinates: &[f64], gap: f64, push: &mut [f64]) -> bool {
        let column = index_of(coordinates[node * 2], self.min_x, self.cell, self.columns);
        let row = index_of(coordinates[node * 2 + 1], self.min_y, self.cell, self.rows);
        let mut crowded = false;
        for step_row in -1..=1 {
            let neighbour_row = row + step_row;
            if neighbour_row < 0 || neighbour_row >= self.rows {
                continue;
            }
            for step_column in -1..=1 {
                let neighbour_column = column + step_column;
                if neighbour_column < 0 || neighbour_column >= self.columns {
                    continue;
                }
                let slot = (neighbour_row * self.columns + neighbour_column) as usize;
                let from = self.offsets[slot] as usize;
                let to = self.offsets[slot + 1] as usize;
                for index in from..to {
                    let other = self.buckets[index] as usize;
                    if other <= node {
                        continue;
                    }
                    if separate_pair(node, other, coordinates, gap, push) {
                        crowded = true;
                    }
                }
            }
        }
        crowded
    }
}

fn separate_pair(
    node: usize,
    other: usize,
    coordinates: &[f64],
    gap: f64,
    push: &mut [f64],
) -> bool {
    let dx = coordinates[other * 2] - coordinates[node * 2];
    let dy = coordinates[other * 2 + 1] - coordinates[node * 2 + 1];
    let distance = dx.hypot(dy);
    if distance >= gap {
        return false;
    }
    let (unit_x, unit_y) = if distance > f64::EPSILON {
        (dx / distance, dy / distance)
    } else {
        let angle = (node as f64) * GOLDEN_ANGLE % (2.0 * PI);
        (angle.cos(), angle.sin())
    };
    let deficit = (gap - distance) * 0.5;
    push[node * 2] -= unit_x * deficit;
    push[node * 2 + 1] -= unit_y * deficit;
    push[other * 2] += unit_x * deficit;
    push[other * 2 + 1] += unit_y * deficit;
    true
}

fn index_of(value: f64, origin: f64, cell: f64, limit: i64) -> i64 {
    (((value - origin) / cell).floor() as i64).clamp(0, limit - 1)
}

#[cfg(test)]
mod tests {
    use super::separate;

    fn closest(coordinates: &[f64]) -> f64 {
        let count = coordinates.len() / 2;
        let mut closest = f64::INFINITY;
        for left in 0..count {
            for right in (left + 1)..count {
                let dx = coordinates[left * 2] - coordinates[right * 2];
                let dy = coordinates[left * 2 + 1] - coordinates[right * 2 + 1];
                closest = closest.min(dx.hypot(dy));
            }
        }
        closest
    }

    #[test]
    fn pushes_overlapping_notes_apart() {
        let mut coordinates = vec![0.0, 0.0, 0.05, 0.0, -0.03, 0.02];

        separate(&mut coordinates, 0.6, 64);

        assert!(
            closest(&coordinates) > 0.55,
            "still crowded: {}",
            closest(&coordinates)
        );
    }

    #[test]
    fn parts_notes_that_share_one_spot() {
        let mut coordinates = vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        separate(&mut coordinates, 0.6, 64);

        assert!(closest(&coordinates) > 0.0, "notes stayed on one spot");
    }

    #[test]
    fn leaves_a_roomy_layout_untouched() {
        let mut coordinates = vec![0.0, 0.0, 3.0, 0.0, 0.0, 3.0];
        let before = coordinates.clone();

        separate(&mut coordinates, 0.6, 32);

        assert_eq!(coordinates, before);
    }

    #[test]
    fn keeps_the_result_the_same_on_every_run() {
        let build = || {
            (0..80)
                .map(|slot: usize| ((slot * 7919) % 41) as f64 * 0.02)
                .collect::<Vec<f64>>()
        };
        let mut first = build();
        let mut second = build();

        separate(&mut first, 0.6, 24);
        separate(&mut second, 0.6, 24);

        assert_eq!(first, second);
    }

    #[test]
    fn one_note_is_left_alone() {
        let mut coordinates = vec![1.0, 2.0];

        separate(&mut coordinates, 0.6, 8);

        assert_eq!(coordinates, vec![1.0, 2.0]);
    }
}
