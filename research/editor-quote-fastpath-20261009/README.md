# Feature-preserving Quote Scan Fastpath

**Goal:** Eliminate unnecessary full-document scans on every CodeMirror transaction while preserving all BookCallout and ReaderQuote behavior.

Original Aquilum commit: `a2ac434ba70c189a5e96e8712ec781ca608ad15c`.

## Design

A per-editor CodeMirror `StateField` tracks conservative presence flags for book and reader quote constructs. On initial document load it scans all lines once. After each immutable `Text` edit it scans only modified lines, unless the flag has already become true. Flags stay true until editor reconstruction, so they never turn false incorrectly when the construct moves or is deleted. `WeakMap<Text, Presence>` lets the existing `findBookCallouts` and `findReaderQuotes` functions return immediately only when absence is **proven** for this exact `Text`. If a document has not registered a hint (standalone tests, other application contexts), both scanners retain their original behavior.

This is not removal of formatting features; it is a negative fast path for documents without these specialized constructs.

## Verification

1. Vitest: absent constructs, insertion character-by-character, changed-line join, Cyrillic, legacy reader links, deletion, real quotes, standalone Text.
2. TypeScript `tsc --noEmit`.
3. Windows native Tauri + WebView2 with identical 5 MB Markdown corpus, 120 CodeMirror insertions, scroll, reopen cycles and process memory.
4. Two baseline runs alternating with two fastpath runs on the same runner and binary. Mode comes from a local loopback endpoint, verified by the renderer before reporting.
5. Compare absolute p50/p95/p99 typing delays and RAM; avoid extrapolating from one note to every workload.

## Limitations

Documents that actually contain supported quote constructs retain the original full scanning cost; a second-stage *positive-case* incremental index would be needed. Over-approximation after deleting a quote is correct but not maximally efficient. Windows summed Working Set is not unique physical RAM. A test-only localStorage toggle and synchronous loopback mode endpoint do not belong in an upstream patch; an upstream proposal would register the field unconditionally.
