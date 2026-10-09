# Real Aquilum source integration

The integration/aquilum-perf branch holds a complete downstream checkout of Freaction/Aquilum, not only a collection of benchmarks.

The import workflow clones the original source at a2ac434ba70c189a5e96e8712ec781ca608ad15c, including Foliate JS, and copies it into the current repository. Original GitHub release workflows are intentionally excluded, the license and attributions are retained, and the submodule source is materialized as regular files.

Production changes applied to the source:
1. integration/patches/quote/apply.py: CodeMirror quote-scanner fast path; preserves support for book and reader quote markup.
2. integration/patches/search/apply_sparse.py: sparse Unicode-offset map reduces transient search RAM.

Benchmark evidence:
- Real Windows CodeMirror quote fast path A/B, 5 MB note: https://github.com/Lalalalendia/aqulium/actions/runs/37937032734
- Whole frontend suite on production-shaped patch, 663 passing: https://github.com/Lalalalendia/aqulium/actions/runs/37939097446
- Rust search sparse memory A/B: https://github.com/Lalalalendia/aqulium/actions/runs/37934492262
- Unicode memory stress: https://github.com/Lalalalendia/aqulium/actions/runs/37935606966

After the workflow pushes the materialized source, actual source files are visible in aquilum-app/. The integration branch stays separate from main until checked. No upstream PR has been submitted, and rar2 is untouched.
