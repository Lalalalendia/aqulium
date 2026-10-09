#!/usr/bin/env python3
"""Install a sparse, lossless offset map in the disposable pinned Aquilum checkout."""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
file = root / 'aquilum-app/core/src/search/matching.rs'
source = file.read_text(encoding='utf-8')
signature = 'struct NormalizedSpan {'
assert source.count(signature) == 1, "unexpected upstream NormalizedText representation"
head, unused_original = source.split(signature, 1)
replacement = r'''
// An ordinary normalized scalar with an unchanged UTF-8 byte width needs
// no per-character mapping: all offsets in the surrounding interval differ
// by one constant shift. This explicitly stores exceptions only.
#[derive(Clone, Copy)]
struct OffsetException {
    normalized_start: usize,
    normalized_end: usize,
    original_start: usize,
    original_end: usize,
}

struct NormalizedText {
    value: String,
    exceptions: Vec<OffsetException>,
}

impl NormalizedText {
    fn new(value: &str) -> Self {
        let mut normalized = String::with_capacity(value.len());
        let mut exceptions = Vec::new();
        for (original_start, character) in value.char_indices() {
            let original_end = original_start + character.len_utf8();
            let normalized_start = normalized.len();
            let mut emitted = 0usize;
            for lowered in character.to_lowercase() {
                decompose_canonical(lowered, |folded| {
                    if !is_combining_mark(folded) {
                        normalized.push(folded);
                        emitted += 1;
                    }
                });
            }
            let normalized_end = normalized.len();
            // A one-to-one scalar with equal byte width has a one-to-one
            // offset mapping, even if case-folding changed the glyph.
            if emitted != 1 || normalized_end - normalized_start != original_end - original_start {
                exceptions.push(OffsetException {
                    normalized_start,
                    normalized_end,
                    original_start,
                    original_end,
                });
            }
        }
        Self { value: normalized, exceptions }
    }

    fn span_at(&self, offset: usize) -> Option<(usize, usize)> {
        if offset >= self.value.len() { return None; }
        // Skip exceptional original scalars that normalized to zero bytes too.
        let index = self.exceptions.partition_point(|span| span.normalized_end <= offset);
        if let Some(exception) = self.exceptions.get(index) {
            if exception.normalized_start <= offset {
                return Some((exception.original_start, exception.original_end));
            }
        }

        // Between exceptional scalars, normalized and original UTF-8 offsets
        // differ by the constant shift established by the previous exception.
        let (previous_original_end, previous_normalized_end) = if index > 0 {
            let previous = &self.exceptions[index - 1];
            (previous.original_end, previous.normalized_end)
        } else { (0, 0) };

        let mut scalar_start = offset;
        while !self.value.is_char_boundary(scalar_start) {
            scalar_start -= 1;
        }
        let scalar_end = scalar_start + self.value[scalar_start..].chars().next()?.len_utf8();

        let (start, end) = if previous_original_end >= previous_normalized_end {
            let delta = previous_original_end - previous_normalized_end;
            (scalar_start.checked_add(delta)?, scalar_end.checked_add(delta)?)
        } else {
            let delta = previous_normalized_end - previous_original_end;
            (scalar_start.checked_sub(delta)?, scalar_end.checked_sub(delta)?)
        };
        Some((start, end))
    }

    fn original_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let first = self.span_at(start)?;
        let last = self.span_at(end.saturating_sub(1))?;
        Some((first.0, last.1))
    }
}

#[cfg(all(test, windows))]
#[path = "matching_sparse_tests.rs"]
mod sparse_tests;
'''
file.write_text(head + replacement, encoding='utf-8')
(file.parent / 'matching_sparse_tests.rs').write_bytes(
    (Path(__file__).parent / 'matching_sparse_tests.rs').read_bytes()
)
print('Installed a sparse UTF-8 offset map; no per-character offset array')
