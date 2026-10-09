//! Compare RAM footprint of Aquilum's actual search NormalizedText with compact u32 spans.
//! The compact version is experimental: a production implementation needs >4GiB fallback.
use super::{NormalizedText, find_prefix_matches, next_char_boundary};
use super::{decompose_canonical, is_combining_mark};
use std::ffi::c_void;
use std::time::Instant;

#[derive(Clone, Copy)]
struct CompactSpan {
    normalized_start: u32,
    normalized_end: u32,
    original_start: u32,
    original_end: u32,
}
struct CompactNormalizedText {
    value: String,
    spans: Vec<CompactSpan>,
}
impl CompactNormalizedText {
    fn new(value: &str) -> Self {
        assert!(value.len() < u32::MAX as usize, "test-only compact prototype requires <4GiB");
        let mut normalized = String::with_capacity(value.len());
        let mut spans = Vec::with_capacity(value.chars().count());
        for (original_start, character) in value.char_indices() {
            let original_end = original_start + character.len_utf8();
            for lowered in character.to_lowercase() {
                decompose_canonical(lowered, |folded| {
                    if is_combining_mark(folded) { return; }
                    let normalized_start = normalized.len();
                    normalized.push(folded);
                    spans.push(CompactSpan {
                        normalized_start: normalized_start as u32,
                        normalized_end: normalized.len() as u32,
                        original_start: original_start as u32,
                        original_end: original_end as u32,
                    });
                });
            }
        }
        Self { value: normalized, spans }
    }
    fn span_at(&self, offset: usize) -> Option<&CompactSpan> {
        let offset = u32::try_from(offset).ok()?;
        let index = self.spans.partition_point(|span| span.normalized_end <= offset);
        self.spans.get(index).filter(|span| span.normalized_start <= offset)
    }
    fn original_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let first = self.span_at(start)?;
        let last = self.span_at(end.saturating_sub(1))?;
        Some((first.original_start as usize, last.original_end as usize))
    }
}
fn compact_find_prefix_matches(text: &str, terms: &[String]) -> Vec<(usize, usize, usize)> {
    let normalized = CompactNormalizedText::new(text);
    let mut matches = Vec::new();
    for (index, term) in terms.iter().enumerate() {
        let mut from = 0;
        while from < normalized.value.len() {
            let Some(relative) = normalized.value[from..].find(term) else { break; };
            let start = from + relative;
            let end = start + term.len();
            let word_start = normalized.value[..start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_alphanumeric());
            if word_start {
                if let Some(original) = normalized.original_range(start, end) {
                    matches.push((original.0, original.1, index));
                }
            }
            from = next_char_boundary(&normalized.value, start);
        }
    }
    matches.sort_unstable_by_key(|range| range.0);
    matches.dedup_by_key(|range| range.0);
    matches
}

#[repr(C)]
#[derive(Default)]
struct ProcessMemoryCountersEx {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_usage: usize,
}
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
}
#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut ProcessMemoryCountersEx, size: u32) -> i32;
}
fn memory() -> (f64, f64, f64) {
    let mut x = ProcessMemoryCountersEx {
        cb: std::mem::size_of::<ProcessMemoryCountersEx>() as u32,
        ..Default::default()
    };
    let size = x.cb;
    assert_ne!(unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut x, size) }, 0);
    let mib = |v: usize| v as f64 / 1_048_576.;
    (mib(x.working_set_size), mib(x.private_usage), mib(x.peak_working_set_size))
}
fn verify_unicode_correctness() {
    let texts = [
        "Alpha ALPHA alpha-omega and alpha_omega.",
        "Тест ТЕСТ тест; Русский РУССКИЙ.",
        "İstanbul: I\u{307}stanbul, ístanbul, ístanbul.",
        "Ångström Аngstrom and e\u{301}cole; Straße STRASSE.",
        "ǅ, Νίκος, ΟΣ, ẞ, Å, ñ, ไทย and 中文.",
        "# Heading\n[[Page 1]] á\nTitle and text",
    ];
    let terms = vec!["a".into(), "alpha".into(), "тест".into(), "рус".into(),
                     "i".into(), "angstrom".into(), "стра".into(), "e".into()];
    for text in texts {
        let expected = find_prefix_matches(text, &terms);
        let observed = compact_find_prefix_matches(text, &terms);
        assert_eq!(expected, observed, "Different Unicode matching for input {text:?}");
    }
}
#[test]
fn ram_lab_unicode_parity() {
    verify_unicode_correctness();
}
#[test]
#[ignore = "run each baseline and compact in separate cargo test processes"]
fn ram_lab_snapshot_search() {
    verify_unicode_correctness();
    let variant = std::env::var("AQUILUM_SEARCH_RAM_VARIANT").expect("Set variant: baseline or compact");
    assert!(variant == "baseline" || variant == "compact");
    let bytes = std::env::var("AQUILUM_MATCH_SIZE_BYTES").ok()
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(5_000_000);
    let paragraph = "## Heading\nAlpha beta gamma, local Markdown search. Тестовый текст, русский язык. A short quote. Long paragraph with ordinary words and 12345.\n";
    let text = paragraph.repeat(bytes / paragraph.len() + 1);
    let input_size = text.len();
    let before = memory();
    let start = Instant::now();
    enum Variant { Baseline(NormalizedText), Compact(CompactNormalizedText) }
    let normalized = if variant == "baseline" {
        Variant::Baseline(NormalizedText::new(&text))
    } else {
        Variant::Compact(CompactNormalizedText::new(&text))
    };
    let ms = start.elapsed().as_secs_f64() * 1000.;
    let after = memory();
    let (nspans, normalized_bytes, capacity, bytes_per_span, first_range) = match &normalized {
        Variant::Baseline(n) => (
            n.spans.len(), n.value.len(), n.spans.capacity(), std::mem::size_of::<super::NormalizedSpan>(),
            n.original_range(0, 1)
        ),
        Variant::Compact(n) => (
            n.spans.len(), n.value.len(), n.spans.capacity(), std::mem::size_of::<CompactSpan>(),
            n.original_range(0, 1)
        ),
    };
    assert_eq!(first_range, Some((0, 1)));
    std::hint::black_box(&normalized);
    println!(
        "AQUILUM_MATCH_RAM variant={} input_bytes={} norm_bytes={} spans={} capacity={} bytes_per_span={} \
        reserved_span_mib={:.3} build_ms={:.3} ws_before_mib={:.3} ws_after_mib={:.3} \
        ws_delta_mib={:.3} private_before_mib={:.3} private_after_mib={:.3} private_delta_mib={:.3} peak_ws_mib={:.3}",
        variant, input_size, normalized_bytes, nspans, capacity, bytes_per_span,
        (capacity * bytes_per_span) as f64 / 1_048_576., ms,
        before.0, after.0, after.0 - before.0,
        before.1, after.1, after.1 - before.1, after.2
    );
    // Keep the live buffer allocated through the memory snapshot.
    drop(normalized);
}
