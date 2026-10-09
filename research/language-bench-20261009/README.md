# Aquilum performance question: TypeScript vs ReScript vs Rust/Wasm

This experiment answers whether rewriting frontend code in ReScript (or moving hot loops to Rust/Wasm) would speed up *actual Aquilum operations*. It does NOT modify production Aquilum.

## What we compare

One **identical line-oriented lexical prefilter** derived from the real quoteScanPresence, isBookCalloutHeader and isReaderQuoteHeader code: on each Markdown line it skips optional ASCII leading spaces/tabs, checks for '>', skips spaces/tabs again, and counts ASCII-case-insensitive [!book] and [!quote] headers. All three implementations return the same encoded counts. The prefilter deliberately does NOT validate complete book quotes or legacy reader links.

Source:
- src/scan.ts : handwritten, idiomatic TypeScript
- src/QuoteScan.res : actual ReScript source compiled to JavaScript with ReScript 12.3.1
- rust/src/lib.rs : actual Rust 2021 compiled to wasm32 via wasm-bindgen and wasm-pack, called through Node JS

The ReScript implementation does the work itself; it is NOT a thin wrapper around TypeScript. Rust/Wasm is also not a native Rust benchmark: it is called from JS, passing the full string across the Wasm boundary on every run.

## Controls

- One identical deterministic generator creates mixed Cyrillic/English, quote-dense, and large 1/5/20 MB Markdown corpora; reported sizes are exact UTF-8 byte lengths.
- Independent regex oracle validates all three scanners on ASCII and Unicode, whitespace, marker case, quote-free lines, inline false positives and CRLF, plus whole test corpora.
- All three languages are optimized release builds. TypeScript and ReScript are bundled/minified with the same esbuild version and settings; Rust/Wasm is a release WebAssembly module plus required JavaScript glue.
- Each language/corpus runs in a separate Node 22 process, 9 warmups and 31 timed synchronous iterations, then repeated in alternate language order. No network, disk, or rendering work inside timed loops.
- Times include actual Rust/Wasm JS-to-UTF8 encoding and copying (the costs incurred in the real browser); no artificial zero-copy advantage.
- Actual code size: raw bytes and gzip bytes for TS, ReScript and Wasm+glue.
- Process RAM: RSS high-water, JS heap after forced GC, external memory. Additional V8 sampled JS heap allocations per call are exploratory and omit Wasm linear memory/native allocations.

### Limitations

- The benchmark is a narrow kernel, NOT a whole-app migration or an evaluation of full CodeMirror rendering and plugin compatibility.
- Node 22/V8 is useful for a comparable host but not identical to WebView2; browser overhead remains.
- Sampled JS allocations and RSS can be dominated by GC/retained pages, and they are NOT measurements of unique resident memory or total Rust allocations.
- Rust works over UTF-8 bytes, JS over UTF-16 code units, but both correctly recognize ASCII syntax regardless of surrounding Unicode.
- Comparing two short-run samples is a hypothesis test, not proof of consistent cross-device wins.
- ReScript compiler speed for this one-file experiment is not evidence it would compile the entire Aquilum faster.

## Outputs

The GitHub Actions workflow compiles and runs the three variants, then writes:
- output/results.csv (one aggregated row per language/corpus)
- output/results.json (every isolated process, memory counters and exact code sizes)
- output/results.md (human-readable Markdown comparison)
- output/build-info.txt (compiler versions and source SHA)

Results are committed only to the experiment branch, and archived as an Actions artifact. The production main branch and rar2 are unchanged.

## Decision rule

Moving this class of code to ReScript solely for performance would require a *meaningful and repeatable* decrease in end-to-end latency or memory without a larger bundled dependency footprint. Otherwise keep TypeScript and focus on algorithmic optimizations.

Consider Rust/Wasm only for a specific CPU-heavy workload when its measured speedup survives input transfer, improves responsiveness beyond ~16ms frame budget, and does not introduce unacceptable linear-memory/bundle startup costs. Whole-application rewrites are not justified by this test.
