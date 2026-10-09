pub const COARSE_MERGE_THRESHOLD: usize = 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineHunk {
    pub from: usize,
    pub to: usize,
    pub lines: Vec<String>,
}

pub struct LineDiff {
    pub hunks: Vec<LineHunk>,
    pub coarse: bool,
}

pub fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

struct CommonSubsequenceTable {
    columns: usize,
    cells: Vec<u32>,
}

impl CommonSubsequenceTable {
    fn build(left: &[&str], right: &[&str]) -> Self {
        let columns = right.len() + 1;
        let mut table = Self { columns, cells: vec![0; (left.len() + 1) * columns] };
        for row in (0..left.len()).rev() {
            for column in (0..right.len()).rev() {
                let value = if left[row] == right[column] {
                    table.at(row + 1, column + 1) + 1
                } else {
                    table.at(row + 1, column).max(table.at(row, column + 1))
                };
                table.cells[row * columns + column] = value;
            }
        }
        table
    }

    fn at(&self, row: usize, column: usize) -> u32 {
        self.cells[row * self.columns + column]
    }
}

fn empty_hunk(at: usize) -> LineHunk {
    LineHunk { from: at, to: at, lines: Vec::new() }
}

fn aligned_hunks(base: &[&str], next: &[&str], offset: usize) -> Vec<LineHunk> {
    let table = CommonSubsequenceTable::build(base, next);
    let mut hunks = Vec::new();
    let mut open: Option<LineHunk> = None;
    let (mut base_index, mut next_index) = (0, 0);

    while base_index < base.len() && next_index < next.len() {
        if base[base_index] == next[next_index] {
            hunks.extend(open.take());
            base_index += 1;
            next_index += 1;
        } else if table.at(base_index + 1, next_index) >= table.at(base_index, next_index + 1) {
            open.get_or_insert_with(|| empty_hunk(offset + base_index)).to = offset + base_index + 1;
            base_index += 1;
        } else {
            open.get_or_insert_with(|| empty_hunk(offset + base_index))
                .lines
                .push(next[next_index].to_owned());
            next_index += 1;
        }
    }
    if base_index < base.len() {
        open.get_or_insert_with(|| empty_hunk(offset + base_index)).to = offset + base.len();
    }
    if next_index < next.len() {
        open.get_or_insert_with(|| empty_hunk(offset + base_index))
            .lines
            .extend(next[next_index..].iter().map(|line| (*line).to_owned()));
    }
    hunks.extend(open);
    hunks
}

pub fn line_diff(base: &[&str], next: &[&str]) -> LineDiff {
    let limit = base.len().min(next.len());
    let head = (0..limit).take_while(|&index| base[index] == next[index]).count();
    let tail = (0..limit - head)
        .take_while(|&index| base[base.len() - 1 - index] == next[next.len() - 1 - index])
        .count();

    let base_middle = &base[head..base.len() - tail];
    let next_middle = &next[head..next.len() - tail];
    if base_middle.is_empty() && next_middle.is_empty() {
        return LineDiff { hunks: Vec::new(), coarse: false };
    }
    if base_middle.len() > COARSE_MERGE_THRESHOLD || next_middle.len() > COARSE_MERGE_THRESHOLD {
        let lines = next_middle.iter().map(|line| (*line).to_owned()).collect();
        return LineDiff {
            hunks: vec![LineHunk { from: head, to: base.len() - tail, lines }],
            coarse: true,
        };
    }
    LineDiff { hunks: aligned_hunks(base_middle, next_middle, head), coarse: false }
}

pub fn apply_hunks(base: &[&str], hunks: &[LineHunk], from: usize, to: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut cursor = from;
    for hunk in hunks {
        result.extend(base[cursor..hunk.from].iter().map(|line| (*line).to_owned()));
        result.extend(hunk.lines.iter().cloned());
        cursor = hunk.to;
    }
    result.extend(base[cursor..to].iter().map(|line| (*line).to_owned()));
    result
}
