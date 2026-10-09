#!/usr/bin/env python3
"""Experimental, correct-by-construction packed-span implementation for pinned Aquilum."""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
path = root / "aquilum-app/core/src/search/matching.rs"
original = path.read_text(encoding="utf-8")
anchor = "struct NormalizedSpan {"
assert original.count(anchor) == 1, "Source signature drifted"
assert original.rstrip().endswith("}"), "Unexpected trailing source"
prefix, _old = original.split(anchor, 1)
new = r'''
#[derive(Clone, Copy)]
struct NormalizedSpan {
    normalized_start: usize,
    normalized_end: usize,
    original_start: usize,
    original_end: usize,
}

#[derive(Clone, Copy)]
struct CompactSpan {
    normalized_start: u32,
    normalized_end: u32,
    original_start: u32,
    original_end: u32,
}

// Never truncate huge offsets. Promote to the original width if a document
// or its Unicode-normalized representation crosses the 32-bit boundary.
enum SpanMap {
    Compact(Vec<CompactSpan>),
    Wide(Vec<NormalizedSpan>),
}

impl SpanMap {
    fn push(&mut self, span: NormalizedSpan) {
        match self {
            Self::Compact(spans)
                if span.normalized_end <= u32::MAX as usize
                    && span.original_end <= u32::MAX as usize =>
            {
                spans.push(CompactSpan {
                    normalized_start: span.normalized_start as u32,
                    normalized_end: span.normalized_end as u32,
                    original_start: span.original_start as u32,
                    original_end: span.original_end as u32,
                });
            }
            Self::Compact(spans) => {
                let mut wide = Vec::with_capacity(spans.len().saturating_add(1));
                wide.extend(spans.iter().map(|small| NormalizedSpan {
                    normalized_start: small.normalized_start as usize,
                    normalized_end: small.normalized_end as usize,
                    original_start: small.original_start as usize,
                    original_end: small.original_end as usize,
                }));
                wide.push(span);
                *self = Self::Wide(wide);
            }
            Self::Wide(spans) => spans.push(span),
        }
    }

    fn span_at(&self, offset: usize) -> Option<NormalizedSpan> {
        match self {
            Self::Compact(spans) => {
                let compact_offset = u32::try_from(offset).ok()?;
                let index = spans.partition_point(|span| span.normalized_end <= compact_offset);
                spans.get(index)
                    .filter(|span| span.normalized_start <= compact_offset)
                    .map(|span| NormalizedSpan {
                        normalized_start: span.normalized_start as usize,
                        normalized_end: span.normalized_end as usize,
                        original_start: span.original_start as usize,
                        original_end: span.original_end as usize,
                    })
            }
            Self::Wide(spans) => {
                let index = spans.partition_point(|span| span.normalized_end <= offset);
                spans.get(index)
                    .filter(|span| span.normalized_start <= offset)
                    .copied()
            }
        }
    }
}

struct NormalizedText {
    value: String,
    spans: SpanMap,
}

impl NormalizedText {
    fn new(value: &str) -> Self {
        let mut normalized = String::with_capacity(value.len());
        // ASCII and Unicode have different ratios of bytes to scalar values.
        // Reserving by characters rather than input bytes avoids overallocating
        // the position mapping for Cyrillic-heavy files.
        let mut spans = if value.len() <= u32::MAX as usize {
            SpanMap::Compact(Vec::with_capacity(value.chars().count()))
        } else {
            SpanMap::Wide(Vec::with_capacity(value.chars().count()))
        };
        for (original_start, character) in value.char_indices() {
            let original_end = original_start + character.len_utf8();
            for lowered in character.to_lowercase() {
                decompose_canonical(lowered, |folded| {
                    if is_combining_mark(folded) {
                        return;
                    }
                    let normalized_start = normalized.len();
                    normalized.push(folded);
                    spans.push(NormalizedSpan {
                        normalized_start,
                        normalized_end: normalized.len(),
                        original_start,
                        original_end,
                    });
                });
            }
        }
        Self { value: normalized, spans }
    }

    fn original_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let first = self.spans.span_at(start)?;
        let last = self.spans.span_at(end.saturating_sub(1))?;
        Some((first.original_start, last.original_end))
    }
}

#[cfg(all(test, windows))]
#[path = "matching_packed_tests.rs"]
mod packed_tests;
'''
path.write_text(prefix + new, encoding="utf-8")
(path.parent / "matching_packed_tests.rs").write_bytes(
    (Path(__file__).parent / "matching_packed_tests.rs").read_bytes()
)
print("Installed bounded compact-span normalizer with automatic wide fallback")
