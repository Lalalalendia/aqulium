# Aquilum actual CodeMirror hotpath ablation - Windows / 5 MB

Diagnostic investigation, not a proposed feature removal. Pinned upstream: Freaction/Aquilum a2ac434ba70c189a5e96e8712ec781ca608ad15c.

Four runs with one compiled binary on one Windows runner:
- baseline: all editor extensions enabled
- no_fullscans: skip findBookCallouts and findReaderQuotes
- no_renumber: omit outlineRenumbering view plugin
- both: combine the two diagnostic changes

The temporary changes are only appropriate for a fixture without book/reader quote callouts and ordered lists. A production optimization must preserve those constructs and incremental updates.

Renderer persists the next mode in WebView2 localStorage after each successful run. Each mode starts as a new native process with separate telemetry output. If the mode does not persist, the workflow aborts. Process-tree teardown is enforced between runs.

Each case creates a deterministic 5 MB Markdown fixture, inserts 120 characters via CodeMirror dispatch at EOF, measures p50/p95/p99 and next animation frame turnaround, performs 140 scroll frames and 10 close/reopen cycles, and measures process tree memory.

Programmatic dispatch is not a direct physical keyboard-to-pixel metric. Compare effect sizes with repeated baseline runs. Removing optional features is NOT a production fix; the test identifies whether to implement incremental scanner caching.
