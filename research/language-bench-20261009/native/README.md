# Native Rust vs TypeScript V8 vs Rust/Wasm on identical Aquilum Markdown scans

This is an isolated experiment in the dedicated repository. It does NOT modify or migrate the production Aquilum editor, and it does not touch rar2 or Freaction/Aquilum.

We reused the exact TypeScript source (src/scan.ts) and deterministic Markdown fixture generator (fixture.mjs) from the previous language comparison. Native Rust and Wasm compile THE SAME kernel, rust/src/kernel.rs. The kernel matches the narrow lexical detection task inspired by Aquilum quoteScanPresence: recognize line-start [!book] and [!quote] headers after optional spaces and '>'. This is not the full CodeMirror parser.

Four tested execution modes:
1. TypeScript compiled/minified to JS by esbuild, executed under Node/V8. Input is a decoded JS string.
2. Rust native optimized release binary, scans the UTF-8 bytes of the identical file already loaded before timing.
3. Rust/Wasm fresh: wasm-bindgen takes the entire JS string, re-encodes as UTF-8 and transfers it on **every invocation**.
4. Rust/Wasm cached: wasm-bindgen PreloadedScanner retains a UTF-8 document in Wasm linear memory; conversion and copying happen ONCE before the timed loop. The preloading latency is recorded separately.

The native and WebAssembly variants are built from the same Rust scanner code, so differences between these two can be attributed to runtime and data boundary (subject to compiler-target optimization). V8 operates on UTF-16 internally while Rust scans UTF-8; it processes the same semantic Unicode document but not the same in-memory representation.

Tests:
- Four modes validated against an independent regex oracle on plain English, Russian, emoji, Japanese, case-insensitive markers, CRLF, false positives and blank text.
- Reuse original TypeScript/ReScript/Wasm correctness harness, plus five full-size corpora: 1, 5, 20 MB mixed Markdown, 5 MB dense quote headers, 5 MB ASCII.
- Three subprocesses per variant/fixture with execution order permuted; 10 warmups then 81/61/41 measured calls (1/5/20 MB).
- Compare hot-call p50/p95/p99, native per-scan allocation calls/bytes (instrumented Rust global allocator), V8 *sampled* JS allocations, process RSS high-water, retained RSS delta, Wasm external/linear memory, one-time preload latency and executable/module sizes.

Limits:
- This is a synchronous lexical kernel, not a frontend/UI benchmark.
- Node/V8 is not identical to Edge WebView2 but provides a consistent controlled environment.
- Native Rust binary is a separate small CLI with far smaller runtime overhead than a Node process. Interpret absolute RSS separately from incremental input/algorithm memory.
- V8 heap allocation sampling excludes Rust and Wasm linear memory. Native allocation counters do not include OS allocations outside Rust's System allocator.
- Cached Wasm changes ownership and memory lifetime: keeping several 20-MB documents resident in Wasm could increase idle app RAM, even if calls are faster.
- Three short-run experiments on a hosted runner are evidence about the algorithm, not a universal assertion that one language is always faster.
- Inputs are generated in native/tmp/ at runtime and NOT committed to Git; reproducibility is through fixture.mjs and the pinned source/versions.

Outcomes are written to native/output/results.json, results.csv, results.md, build-info.txt and raw-run.log. GitHub Actions archives the compiled binary and results, and commits only results back to this experiment branch. main is unchanged.
