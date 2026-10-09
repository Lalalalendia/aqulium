# Aquilum: TypeScript vs ReScript vs Rust/Wasm benchmark

Controlled run of the same lexical quote-header prefilter on real-size Markdown strings. Two separate native Node processes per language / workload; 31 timed calls per process after JIT warmup. Compiler and package versions in build logs.

| Workload | Language | p50 (ms) | p95 (ms) | RSS peak (MiB) | JS heap after GC (MiB) | Sampled JS allocations / call (bytes) | Raw deliverable | Gzip |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| mixed 1 MB | typescript | 0.25226 | 1.66418 | 54.613 | 5.27 | 898 | 570 | 345 |
| mixed 1 MB | rescript | 0.26321 | 1.42594 | 55.2 | 5.27 | 0 | 564 | 334 |
| mixed 1 MB | rustwasm | 0.9513 | 1.61126 | 54.523 | 5.29 | 0 | 17747 | 7394 |
| mixed 5 MB | typescript | 1.24734 | 1.42612 | 59.951 | 10.508 | 410 | 570 | 345 |
| mixed 5 MB | rescript | 1.28648 | 1.47396 | 59.397 | 10.508 | 896.5 | 564 | 334 |
| mixed 5 MB | rustwasm | 4.52318 | 6.55999 | 69.029 | 10.527 | 0 | 17747 | 7394 |
| dense 5 MB | typescript | 2.48393 | 2.72557 | 61.074 | 11.007 | 0 | 570 | 345 |
| dense 5 MB | rescript | 2.76688 | 2.90885 | 60.561 | 11.007 | 0 | 564 | 334 |
| dense 5 MB | rustwasm | 4.68142 | 5.76085 | 69.496 | 11.027 | 0 | 17747 | 7394 |
| mixed 20 MB | typescript | 5.03258 | 6.6887 | 79.049 | 30.193 | 0 | 570 | 345 |
| mixed 20 MB | rescript | 5.21877 | 5.56736 | 79.324 | 30.194 | 0 | 564 | 334 |
| mixed 20 MB | rustwasm | 18.09578 | 18.26598 | 112.927 | 30.213 | 824.5 | 17747 | 7394 |

## Important scope and limits

- All three were checked against an independent regex oracle on mixed-case, Cyrillic, Japanese, emoji, CRLF and typical marker syntax. This is a lexical prefilter; it is not the full CodeMirror quoteScanPresence, isBookCalloutHeader or reader quote parser.
- Rust runs as WebAssembly inside Node, not as a native executable. JS string to UTF-8 Wasm marshaling/copying is counted in every operation.
- The measured p50 is a hot scanner call. It is not actual keyboard-to-pixel UI latency nor whole-app idle memory.
- RSS peak is entire Node process high-water; it includes the common input string and module dependencies. JS allocations come from V8 sampling after timing and exclude native/Wasm linear memory.
- Build size is minified bundled TS or ReScript JS versus WASM binary + required JS glue. Gzip totals are the sum of each shipping file.
- Results are two runner-local runs, not a statistical guarantee across CPUs or browsers. Investigate WebView2 separately before any language migration.
