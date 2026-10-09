//! Production-shaped sparse normalization parity and memory A/B.
use super::{find_prefix_matches, next_char_boundary, NormalizedText};
use super::{decompose_canonical, is_combining_mark};
use std::ffi::c_void;
use std::time::Instant;

#[derive(Clone, Copy)]
struct RefSpan {
    normalized_start: usize,
    normalized_end: usize,
    original_start: usize,
    original_end: usize,
}
struct RefNormalized {
    value: String,
    spans: Vec<RefSpan>,
}
impl RefNormalized {
    fn new(value: &str) -> Self {
        let mut normalized = String::with_capacity(value.len());
        let mut spans = Vec::with_capacity(value.len());
        for (original_start, character) in value.char_indices() {
            let original_end = original_start + character.len_utf8();
            for lowered in character.to_lowercase() {
                decompose_canonical(lowered, |folded| {
                    if is_combining_mark(folded) { return; }
                    let normalized_start = normalized.len();
                    normalized.push(folded);
                    spans.push(RefSpan {
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
    fn span_at(&self, offset: usize) -> Option<&RefSpan> {
        let index = self.spans.partition_point(|span| span.normalized_end <= offset);
        self.spans.get(index).filter(|span| span.normalized_start <= offset)
    }
    fn original_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let first = self.span_at(start)?;
        let last = self.span_at(end.saturating_sub(1))?;
        Some((first.original_start, last.original_end))
    }
}
fn reference_matches(text: &str, terms: &[String]) -> Vec<(usize, usize, usize)> {
    let normalized = RefNormalized::new(text);
    let mut matches = Vec::new();
    for (index, term) in terms.iter().enumerate() {
        let mut from = 0;
        while from < normalized.value.len() {
            let Some(relative) = normalized.value[from..].find(term) else { break; };
            let start = from + relative;
            let end = start + term.len();
            let word_start = normalized.value[..start].chars().next_back()
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

#[test]
fn sparse_matches_original_unicode_positions() {
    let samples = [
        "Alpha ALPHA alpha-omega and alpha_omega.",
        "Тест ТЕСТ тест; Русский РУССКИЙ.",
        "İstanbul: I\u{307}stanbul, i\u{301}stanbul, ístanbul.",
        "Ångström, e\u{301}cole; Straße STRASSE.",
        "# Heading\n[[Page 1]] a\u{301}\nTitle and text",
        "é́e ǅ, Νίκος, ΟΣ, ẞ, Å, ñ, ไทย and 中文.",
        "\u{0301}\u{0302} 𝒜É — \u{0301}Новый мир",
        "\u{AC00} 가나다 바다 한국어 \u{0301} TEXT",
    ];
    let terms: Vec<String> = [
        "a", "alpha", "тест", "рус", "i", "angstrom", "e", "стра", "н", "σ", "title", "中", "ᄀ",
    ].iter().map(|x| x.to_string()).collect();
    for text in samples {
        let expected = reference_matches(text, &terms);
        let actual = find_prefix_matches(text, &terms);
        assert_eq!(expected, actual, "Unicode mismatch for text {text:?}");
    }

    let alphabet = [
        "а", "Б", "в", "Ё", "é", "a", "A", "İ", "i", "ß", "ǅ", "\u{0301}",
        "Ω", "ς", "中", "文", "ก", "\u{AC00}", ".", "-", " ", "\n", "[", "]", "ᄀ",
    ];
    let mut seed = 0x8b_1642_2f45e5dca1_u64;
    for case in 0..80 {
        let mut text = String::new();
        for _ in 0..1000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            text.push_str(alphabet[seed as usize % alphabet.len()]);
        }
        assert_eq!(
            reference_matches(&text, &terms),
            find_prefix_matches(&text, &terms),
            "Randomized Unicode case {case}"
        );
    }
}

#[test]
fn sparse_omits_identity_scalars_and_maps_accents() {
    let text = "Hello Привет \u{0301}éİ\u{AC00} 世界";
    let ref_map = RefNormalized::new(text);
    let sparse = NormalizedText::new(text);
    assert_eq!(sparse.value, ref_map.value);
    assert!(sparse.exceptions.len() < ref_map.spans.len());
    for (index, ch) in sparse.value.char_indices() {
        let (_, original_len) = sparse.original_range(index, index + ch.len_utf8())
            .map(|(a, b)| (a, b - a)).expect("nonempty mapping");
        assert!(original_len > 0);
        assert_eq!(
            ref_map.original_range(index, index + ch.len_utf8()),
            sparse.original_range(index, index + ch.len_utf8()),
            "Mapped offset {index} disagrees"
        );
    }
    assert_eq!(NormalizedText::new("ASCII and русский текст").exceptions.len(), 0);
}

#[repr(C)]
#[derive(Default)]
struct Counters {
    cb: u32, page_fault_count: u32,
    peak_working_set_size: usize, working_set_size: usize,
    quota_peak_paged_pool_usage: usize, quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize, quota_non_paged_pool_usage: usize,
    pagefile_usage: usize, peak_pagefile_usage: usize, private_usage: usize,
}
#[link(name = "kernel32")]
extern "system" { fn GetCurrentProcess() -> *mut c_void; }
#[link(name = "psapi")]
extern "system" {
    fn GetProcessMemoryInfo(process: *mut c_void, counters: *mut Counters, cb: u32) -> i32;
}
fn counters() -> (f64, f64) {
    let mut x = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
    let n = x.cb;
    assert_ne!(unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut x, n) }, 0);
    (x.working_set_size as f64 / 1_048_576., x.private_usage as f64 / 1_048_576.)
}
#[test]
#[ignore = "A/B: run original and sparse in separate processes"]
fn sparse_memory_and_time_profile() {
    let variant = std::env::var("AQUILUM_SPARSE_VARIANT").expect("baseline/sparse mode");
    assert!(variant == "baseline" || variant == "sparse");
    let size = std::env::var("AQUILUM_SPARSE_SIZE").ok()
        .and_then(|s| s.parse::<usize>().ok()).unwrap_or(5_000_000);
    let corpus = std::env::var("AQUILUM_SPARSE_CORPUS").unwrap_or_else(|_| "mixed".into());
    let paragraph = match corpus.as_str() {
        "ascii" => "Alpha beta gamma. The quick brown fox jumps over a lazy dog.\n",
        "cyrillic" => "Русский текст. Ёлка, йота, чтение и письмо. Ещё много предложений.\n",
        "accented" => "éìêïÅàäçñİÖÙe\u{0301} éÈÍÉñöüİ çède, señor café!\n",
        "hangul" => "가나다라마바사 아자차카타파하 한국어 테스트 문자열과 한글 문서\n",
        "mixed" => "## Heading\nAlpha beta gamma, local Markdown search. Тестовый текст, русский язык. A short quote. Long paragraph with ordinary words and 12345.\n",
        _ => panic!("Unknown RAM stress corpus: {corpus}"),
    };
    let input = paragraph.repeat(size / paragraph.len() + 1);
    let baseline = counters();
    let clock = Instant::now();
    enum Sample { Original(RefNormalized), Sparse(NormalizedText) }
    let result = match variant.as_str() {
        "baseline" => Sample::Original(RefNormalized::new(&input)),
        _ => Sample::Sparse(NormalizedText::new(&input)),
    };
    let ms = clock.elapsed().as_secs_f64() * 1000.;
    let after = counters();
    let (normalized_bytes, positions, capacity_bytes) = match &result {
        Sample::Original(r) => (
            r.value.len(), r.spans.len(),
            r.spans.capacity() * std::mem::size_of::<RefSpan>()
        ),
        Sample::Sparse(r) => (
            r.value.len(), r.exceptions.len(),
            r.exceptions.capacity() * std::mem::size_of::<super::OffsetException>()
        ),
    };
    std::hint::black_box(&result);
    println!(
        "AQUILUM_SPARSE_RAM variant={} corpus={} input_bytes={} normalized_bytes={} mapping_entries={} \
         mapping_capacity_mib={:.3} build_ms={:.3} ws_before_mib={:.3} ws_after_mib={:.3} \
         ws_delta_mib={:.3} private_before_mib={:.3} private_after_mib={:.3} private_delta_mib={:.3}",
        variant, corpus, input.len(), normalized_bytes, positions,
        capacity_bytes as f64 / 1_048_576., ms, baseline.0, after.0,
        after.0-baseline.0, baseline.1, after.1, after.1-baseline.1
    );

    // Repeat construction enough times to obtain a useful warm-run distribution.
    // Peak RAM above was measured before these additional runs.
    let mut timings = Vec::with_capacity(11);
    timings.push(ms);
    drop(result);
    for _ in 0..10 {
        let started = Instant::now();
        if variant == "baseline" {
            std::hint::black_box(RefNormalized::new(&input));
        } else {
            std::hint::black_box(NormalizedText::new(&input));
        }
        timings.push(started.elapsed().as_secs_f64() * 1000.);
    }
    timings.sort_by(f64::total_cmp);
    println!(
        "AQUILUM_SPARSE_CPU variant={} corpus={} size={} n={} p50_ms={:.3} p95_ms={:.3}",
        variant, corpus, input.len(), timings.len(),
        timings[timings.len() / 2], timings[(timings.len()-1)*95/100]
    );
}
