# Aquilum: native Rust vs TypeScript V8 vs Rust/Wasm (fresh and preloaded)

Same [!book]/[!quote] Markdown lexical prefilter on exact same text. Three fresh processes per variant/workload, 10 warmups, multiple calls in each process. Hot-call median p50 for each set is aggregated as the median of three per-process medians.

| Text | Variant | p50 (ms) | p95 (ms) | vs TypeScript | Peak RSS (MiB) | RSS growth loaded/warmed (MiB) |
|---|---|---:|---:|---:|---:|---:|
| mixed 1 MB | typescript | 0.268102 | 0.465983 | 1x | 56.93 | 8.207 |
| mixed 1 MB | rust_native | 0.072226 | 0.088736 | 3.712x | 2.922 | 0.977 |
| mixed 1 MB | wasm_fresh | 0.936856 | 1.100783 | 0.286x | 55.313 | 8.336 |
| mixed 1 MB | wasm_cached | 0.242986 | 0.450915 | 1.103x | 55.285 | 8.785 |
| mixed 5 MB | typescript | 1.337928 | 1.383103 | 1x | 61.262 | 13.387 |
| mixed 5 MB | rust_native | 0.384201 | 0.40566 | 3.482x | 6.723 | 4.777 |
| mixed 5 MB | wasm_fresh | 4.585617 | 4.765243 | 0.292x | 69.43 | 22.785 |
| mixed 5 MB | wasm_cached | 1.209307 | 1.657547 | 1.106x | 68.207 | 21.602 |
| dense 5 MB | typescript | 2.36833 | 2.434073 | 1x | 61.77 | 14.04 |
| dense 5 MB | rust_native | 0.601497 | 0.615013 | 3.937x | 6.723 | 4.785 |
| dense 5 MB | wasm_fresh | 4.797173 | 4.990986 | 0.494x | 69.902 | 23.254 |
| dense 5 MB | wasm_cached | 1.444408 | 2.524102 | 1.64x | 68.641 | 21.906 |
| ascii 5 MB | typescript | 1.779406 | 1.831994 | 1x | 64.988 | 16.508 |
| ascii 5 MB | rust_native | 0.487644 | 0.503774 | 3.649x | 6.777 | 4.828 |
| ascii 5 MB | wasm_fresh | 4.416308 | 4.495327 | 0.403x | 77.75 | 31.148 |
| ascii 5 MB | wasm_cached | 1.272675 | 1.651727 | 1.398x | 71.715 | 25.063 |
| mixed 20 MB | typescript | 5.37757 | 5.450417 | 1x | 92.949 | 33.176 |
| mixed 20 MB | rust_native | 1.476938 | 1.530659 | 3.641x | 21.023 | 19.113 |
| mixed 20 MB | wasm_fresh | 18.352612 | 18.443884 | 0.293x | 113.203 | 66.586 |
| mixed 20 MB | wasm_cached | 4.796611 | 4.937606 | 1.121x | 116.422 | 69.852 |

## Code/package size

| Executable/deliverable | Raw bytes | Gzip bytes | Files |
|---|---:|---:|---|
| typescript | 570 | 345 | dist/ts.mjs |
| rust_native | 349504 | 180845 | native/target/release/aquilum_native_scan |
| wasm_fresh | 20846 | 8389 | rust/pkg/aquilum_scan_bench.js, rust/pkg/aquilum_scan_bench_bg.wasm |
| wasm_cached | 20846 | 8389 | rust/pkg/aquilum_scan_bench.js, rust/pkg/aquilum_scan_bench_bg.wasm |

## Correct interpretation

- Native Rust and Wasm use the exact SAME Rust kernel source. TypeScript uses its previous V8 code. Both recognize identical headers across the same Markdown Unicode files.
- Native Rust reads UTF-8 input before timing. TypeScript uses an already-decoded JS UTF-16 string. Wasm-fresh re-encodes/copies that JS string each call; Wasm-cached copies once outside timing and then scans its own retained buffer.
- Preload times and JS external memory are in results.json. Native allocation counters are for timed scanning only; V8 sampled allocation counters refer only to sampled JS heap, not Wasm or Rust allocator.
- Whole-process RSS is not the same as memory consumed by one application module. A native CLI does not run WebView2 or Node; compare speed and incremental RAM separately from baseline process overhead.
- A narrow lexical scan does not prove anything about full CodeMirror rendering, asynchronous IPC, search integration or replacing the entire Aquilum TypeScript frontend.
- RSS and GC can fluctuate between runs; medians from three same-host subprocesses are a focused microbenchmark, not a universal language ranking.
