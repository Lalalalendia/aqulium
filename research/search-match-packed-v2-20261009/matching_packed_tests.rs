//! Regression and RAM tests for the candidate packed-position representation.
use super::{find_prefix_matches, next_char_boundary, NormalizedText, NormalizedSpan, SpanMap};
use super::{decompose_canonical, is_combining_mark};
use std::ffi::c_void;
use std::time::Instant;

#[derive(Clone, Copy)]
struct RefSpan {
    norm_from: usize,
    norm_end: usize,
    original_from: usize,
    original_end: usize,
}
fn old_matches(text: &str, terms: &[String]) -> Vec<(usize, usize, usize)> {
    let mut normalized = String::with_capacity(text.len());
    let mut spans = Vec::<RefSpan>::new();
    for (original_start, character) in text.char_indices() {
        let original_end = original_start + character.len_utf8();
        for lowered in character.to_lowercase() {
            decompose_canonical(lowered, |folded| {
                if is_combining_mark(folded) { return; }
                let start = normalized.len();
                normalized.push(folded);
                spans.push(RefSpan {
                    norm_from: start, norm_end: normalized.len(),
                    original_from: original_start, original_end,
                });
            });
        }
    }
    let lookup = |offset: usize| -> Option<RefSpan> {
        let index = spans.partition_point(|span| span.norm_end <= offset);
        spans.get(index).filter(|span| span.norm_from <= offset).copied()
    };
    let mut matches = Vec::new();
    for (index, term) in terms.iter().enumerate() {
        let mut from = 0;
        while from < normalized.len() {
            let Some(relative) = normalized[from..].find(term) else { break; };
            let start = from + relative;
            let end = start + term.len();
            let word_start = normalized[..start].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
            if word_start {
                if let (Some(first), Some(last)) = (lookup(start), lookup(end.saturating_sub(1))) {
                    matches.push((first.original_from, last.original_end, index));
                }
            }
            from = next_char_boundary(&normalized, start);
        }
    }
    matches.sort_unstable_by_key(|range| range.0);
    matches.dedup_by_key(|range| range.0);
    matches
}
#[test]
fn packed_spans_preserve_unicode_search_offsets() {
    let alphabet = [
        "а", "Б", "в", "Ё", "é", "a", "A", "İ", "i", "ß", "ǅ",
        "́", "Ω", "ς", "中", "文", "\u{0301}", ".", "-", " ", "\n", "[", "]",
    ];
    let terms: Vec<String> = ["a","б","е","i","abc","is","ω","и","index","é","中"]
        .iter().map(|s| (*s).to_owned()).collect();
    let samples = [
        "Alpha ALPHA alpha-omega and alpha_omega.",
        "Тест ТЕСТ тест; Русский РУССКИЙ.",
        "İstanbul: I\u{307}stanbul, i\u{301}stanbul, ístanbul.",
        "Ångström, e\u{301}cole; Straße STRASSE.",
        "# Heading\n[[Page 1]] a\u{301}\nTitle and text",
    ];
    for text in samples {
        assert_eq!(old_matches(text, &terms), find_prefix_matches(text, &terms), "sample text {text:?}");
    }
    let mut state = 0x4a9d_b9f8_u64;
    for _ in 0..20 {
        let mut text = String::new();
        for _ in 0..800 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            text.push_str(alphabet[(state as usize) % alphabet.len()]);
        }
        assert_eq!(old_matches(&text, &terms), find_prefix_matches(&text, &terms), "fuzz case");
    }
}
#[test]
fn packed_spans_promote_instead_of_truncating_large_positions() {
    let mut map = SpanMap::Compact(Vec::new());
    map.push(NormalizedSpan {
        normalized_start: 0, normalized_end: 1,
        original_start: 0, original_end: 1,
    });
    map.push(NormalizedSpan {
        normalized_start: u32::MAX as usize + 4,
        normalized_end: u32::MAX as usize + 5,
        original_start: u32::MAX as usize + 10,
        original_end: u32::MAX as usize + 11,
    });
    assert!(matches!(map, SpanMap::Wide(_)));
    let restored = map.span_at(u32::MAX as usize + 4).expect("wide span exists");
    assert_eq!(restored.original_end, u32::MAX as usize + 11);
}

#[repr(C)]
#[derive(Default)]
struct MemoryEx {
    cb: u32, page_faults: u32,
    peak_working_set: usize, working_set: usize,
    quota_peak_paged: usize, quota_paged: usize,
    quota_peak_nonpaged: usize, quota_nonpaged: usize,
    pagefile_usage: usize, peak_pagefile_usage: usize, private_usage: usize,
}
#[link(name = "kernel32")]
extern "system" { fn GetCurrentProcess() -> *mut c_void; }
#[link(name = "psapi")]
extern "system" { fn GetProcessMemoryInfo(p: *mut c_void, mem: *mut MemoryEx, n: u32) -> i32; }
fn snapshot() -> (f64, f64) {
    let mut m = MemoryEx { cb: std::mem::size_of::<MemoryEx>() as u32, ..Default::default() };
    let n = m.cb;
    assert_ne!(unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut m, n) }, 0);
    (m.working_set as f64 / 1_048_576., m.private_usage as f64 / 1_048_576.)
}

#[test]
#[ignore = "run separately to measure live candidate RAM in a fresh Rust process"]
fn packed_ram_snapshot() {
    let size = std::env::var("AQUILUM_MATCH_SIZE_BYTES").ok()
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(5_000_000);
    let para = "## Heading\nAlpha beta gamma, local Markdown search. Тестовый текст, русский язык. A short quote. Long paragraph with ordinary words and 12345.\n";
    let text = para.repeat(size / para.len() + 1);
    let before = snapshot();
    let start = Instant::now();
    let normalized = NormalizedText::new(&text);
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.;
    let after = snapshot();
    let (kind, entries, allocated) = match &normalized.spans {
        SpanMap::Compact(v) => ("compact", v.len(), v.capacity() * std::mem::size_of_val(&v[0])),
        SpanMap::Wide(v) => ("wide", v.len(), v.capacity() * std::mem::size_of_val(&v[0])),
    };
    std::hint::black_box(&normalized);
    println!(
        "AQUILUM_PACKED_RAM size_bytes={} format={} spans={} allocated_span_mib={:.3} build_ms={:.3} \
        ws_before_mib={:.3} ws_after_mib={:.3} ws_delta_mib={:.3} \
        private_before_mib={:.3} private_after_mib={:.3} private_delta_mib={:.3}",
        text.len(), kind, entries, allocated as f64 / 1_048_576.,
        elapsed_ms, before.0, after.0, after.0-before.0,
        before.1, after.1, after.1-before.1
    );
}
