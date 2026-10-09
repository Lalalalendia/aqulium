# Aquilum Native: memory findings and migration decision

Evidence gathered on real GitHub-hosted Windows runners, 2026-10-09.
These are **experimental branches, not merged into main**. Do not use
prototype editors with valuable live notes.

## 1. Both native UI frameworks can build

- [Iced and GPUI paged shell, shared DocumentHub](https://github.com/Lalalalendia/aqulium/actions/runs/37972110760): Windows release binaries built; common core tests green.
- [GPUI Component real multiline InputState editor](https://github.com/Lalalalendia/aqulium/actions/runs/37975167896): Windows release binary successfully built after resolving compiler API differences.
- GPUI paged shell previously failed to create an HWND on the Windows runner. No RAM claims were made from those readings (its process had not created a functioning GUI).

## 2. Memory comparison of ACTUAL multiline widgets

[Native Windows ABBA (Iced and GPUI multiline)](https://github.com/Lalalalendia/aqulium/actions/runs/37975887605): two runs of each editor at every corpus size, all GUI HWNDs observed, repeated 26 process-memory readings over ~13 seconds, input SHA-256 hashes unchanged.

| UTF-8 Markdown corpus | Iced private commit (MiB) | GPUI multiline private commit (MiB) |
|---|---:|---:|
| 1 MiB, one note | 373.86 / 369.57 | 106.20 / 104.73 |
| 5 MiB, one note | 1671.55 / 1671.46 | 336.73 / 338.55 |
| 20 MiB, one note | 1496.83 (not fully stable), >2562.94 (capped) | 1206.69 / 1202.02 final; 1908-1962 MiB peak during initialization |
| Three 5-MiB notes | >2561 / >2567 (capped) | 422.09 / 418.38 |

The GPUI full InputState model was around **5x smaller** than the Iced
full-document widget on a 5-MiB note. However, GPUI still consumed >1.2 GiB
for a 20-MiB note, which is unacceptable as a general large-document solution.
These are native process private bytes, not total desktop application RAM
including GPU driver allocations.

The test established HWND creation and 13 seconds of process life; it did
**not** prove first-paint timing, real keyboard/IME behavior, crash recovery,
Undo/Redo parity, multi-cursor, Markdown live preview or full feature parity.

## 3. Root cause for Iced's enormous memory

[Headless Iced Content model RAM](https://github.com/Lalalalendia/aqulium/actions/runs/37976250570) constructs text_editor::Content *without a GPU, window or renderer*.

| Input bytes | Private committed process memory |
|---|---:|
| 1 MiB | 329.12 MiB |
| 5 MiB | 1615.61 MiB |
| 20 MiB | Stopped at 2524 MiB before Content returned |

This shows that most of the blow-up occurs *inside the full-document Iced
Content model*, not in WebView2 nor the WGPU GPU cache. Do not take the shortcut
of replacing CodeMirror with a stock whole-document Iced TextEditor.

## 4. Compact, framework-independent viewport

[Indexed Markdown viewport, Linux and Windows](https://github.com/Lalalalendia/aqulium/actions/runs/37976658836): 12 tests passing on both platforms. It stores one UTF-8 String and a Vec of line-byte starts, returns a borrowed visible range, supports Unicode-safe edits, and updates only affected line-boundary indices.

For a 20-MiB / 487,711-line fixture:
- **20,971,530** bytes of Markdown.
- **24,873,218** bytes allocated to text + line-index capacity (~23.7 MiB), excluding allocator and CRDT overhead.
- **2,580** bytes of visible text selected by one zero-copy 60-line view.

This is MODEL capacity, not process RSS; it does not yet prove
keyboard-to-pixel performance. It currently adjusts subsequent line offsets
in O(line count), and global UTF-16-to-UTF-8 conversion is O(document).
A proper production rope/piece-table and incremental UTF-16 summary index are
future optimizations.

The next vertical prototype uses this exact buffer and loads only 80 visible
lines into Iced's editable Content; it must pass Windows GUI and persistence
tests before declaring success.

## 5. Architectural decision

Do not migrate the entire TypeScript UI immediately. Adopt the Strangler
pattern in bounded, reversible slices:
1. Keep the existing reliable Tauri/CodeMirror product active and unchanged.
2. Reuse the one authoritative aquilum-core::DocumentHub (Yrs, SQLite,
   history). Do not create separate competing writers for a live file.
3. Implement a compact native text buffer and virtualized editing viewport.
4. Prove cursor, select, multiline insertions, Unicode/IME, undo/redo,
   exact save/reopen and cross-page edits with real Windows interaction tests.
5. Benchmark 1/5/20 MiB documents and three tabs against the actual
   Tauri/WebView2 full-process baseline, with p50/p95 input latency and private RAM.
6. Only then pick GPUI/Iced/another GPU framework. Engine name is secondary
   to text model and viewport architecture.
7. Postpone EPUB/Foliate replacement until independent CFI/pagination fidelity
   tests exist. Browser components can remain if they are the correct tool.

A separate [WebView2 live-tabs A/B branch](https://github.com/Lalalalendia/aqulium/tree/experiments/live-tabs-ram-20261009) is measuring the baseline. Any candidate that is faster in a microbenchmark but loses data, consumes gigabytes of RAM or lacks IME must not be promoted to production.
