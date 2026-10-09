# Production-shaped fastpath for Aquilum BookCallout/ReaderQuote scans

Base: Freaction/Aquilum at `a2ac434ba70c189a5e96e8712ec781ca608ad15c` (v0.2.6).

An upstream-ready change without any benchmark-specific toggles or local HTTP endpoint. All ordinary CodeMirror editors automatically register the conservative `quoteScanPresence` StateField. Existing standalone `Text` consumers retain original scanner behavior.

- Initial open: full line scan once to find whether specialized quote constructs may exist.
- Editing notes with no such constructs: each transaction examines affected lines only; the full scanners return empty immediately.
- Once a quote is present, the appropriate original full scanner remains active; after deleting all quotes the flag stays true until editor remount to avoid false negatives.
- Cache keyed by immutable CodeMirror `Text` in a WeakMap, preventing accidental retention of old documents.
- New markers typed character-by-character, legacy reader quote syntax, Unicode/Cyrillic, newline merges, and deletion verified by unit tests.

## Evidence

Real full Tauri/WebView2 Windows repeated A/B: [Actions 37937032734](https://github.com/Lalalalendia/aqulium/actions/runs/37937032734). Same 5 MB document:
- Baseline p50 72.2 / 72.7 ms.
- Feature-preserving fastpath p50 51.4 / 50.5 ms.
- Baseline p95 95.9 / 85.7 ms; fastpath 64.3 / 60.6 ms.
- All four real WebView2 scenarios completed.
- 19 relevant Vitest tests and TypeScript build passed.

This candidate branch independently runs the **full frontend suite** and a production Vite build before any proposal to author.

This is a latency optimization; working-set RAM changes were not large enough to prove a reduction on the benchmark machine. Do not claim lower idle RAM.
