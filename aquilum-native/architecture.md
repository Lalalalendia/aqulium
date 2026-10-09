# ADR: Aquilum Native / Strangler Fig migration

Decision recorded 2026-10-09. Goal: determine whether a native Rust UI can
retain all Aquilum behavior and materially lower private RAM/latency.

## Non-negotiable state ownership

Existing storage path:
  Tauri (commands) -> aquilum-core::Core -> DocumentHub / Yrs / SQLite / Tantivy

Native experiment:
  Iced or GPUI -> aquilum-native-common -> the SAME aquilum-core::Core

Never start two independent writers targeting the same live document store. The
experiment uses its own app data directory, distinct from the existing Tauri
profile. When comparing native and Tauri, run on independent copies of the
Markdown files. Avoid multiple prototype processes saving to the same files.

The experimental adapter uses DocumentHub::replace_text for explicit save.
This is NOT an efficient keypress path: full text is read from the editor on
save, and the core may compute a diff against its Yrs replica. The production
editing protocol must preserve CodeMirror transaction deltas or use typed
Rust commands; do not send the whole document on every keypress.

## Unicode

CodeMirror and Yrs store UTF-16 offsets. Rust byte/rope positions are not
UTF-16. A correct UTF-8-byte-to-UTF-16 conversion layer must test:
- Cyrillic; emoji and supplementary planes; combining marks and ZWJ;
- newline joins, backward deletion, selection, insertions and replacement;
- concurrent edits and stale versions, including filesystem watch reconciles.
The initial prototype uses explicit full-text replace/save and does not claim
to solve the incremental coordinate protocol.

## Proposed migration stages

0. Measurement gates and common backend (this branch).
1. UI shell: windows, tabs, panels, search (Rust already owns the data).
2. Editable text model, incremental transactions, persistence and Undo/Redo.
3. Virtualized Markdown view and syntax highlighting (big notes first).
4. Live Preview, tables, links, images and data views. Original TS baseline is
   the behavioral oracle. Every migrated feature must have equivalent tests.
5. Graph visualization and ancillary settings/screens.
6. EPUB reader; Foliate's HTML/CSS, CFI pagination, highlights and annotations
   are high-risk to reimplement; retaining a scoped web renderer is acceptable
   if replacing it harms correctness or performance.
7. Only remove the original Tauri/WebView2 path after feature parity, data
   durability, accessibility and measured wins on supported OSes.

## Review/exit criteria

Bench: same 1/5/20MB notes, 1/3 open tabs, cold/warm launch, p50/p95/p99
keyboard-to-pixel latency, p95 scroll/frame time, app + child private bytes,
Working Set (including external process composition), no unbounded allocation
after 10 tab switches; verify real keyboard/IME usage.

Safety: no lost edits; stale version is rejected; crash/restart roundtrip;
backups/history work; cross-process lock and watcher reconciliation specified.
Target >=30% private-memory reduction is a hypothesis for further testing,
NOT an expected or observed result.

GPUI vs Iced smoke builds are an engineering feasibility gate only. GPUI's
current prototype is a viewer, Iced's a real editing widget, and comparing
their RAM today is invalid.
