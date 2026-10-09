# Aquilum 2026-10-09: Windows RAM / UI-latency experiment

## Scope

Pinned upstream: Freaction/Aquilum at a2ac434ba70c189a5e96e8712ec781ca608ad15c.
Experiment branch: experiments/ram-profile-20261009 in Lalalalendia/aqulium.
Temporary test-only patches are applied inside the GitHub Actions runner checkout; nothing is sent to upstream.

### Native Rust (Yrs / CRDT) memory

- native_ram_profile is an actual aquilum-core release test on Windows.
- Captures Windows working set (resident, includes shared) and PrivateUsage (private committed bytes).
- Milestones: initialized core; one 5 MB note open/closed; 50 reopen cycles; five additional copies open/closed; shutdown.
- In-process high-water marks are not proof of memory leaks: the OS and Rust allocator may retain memory after free.

### Actual desktop process including WebView2

- Builds real Aquilum/Tauri with React and CodeMirror, using an ephemeral test-only Vite build flag.
- Starts a real Windows executable; samples root PID plus recursive child process tree.
- Records separate app vs WebView2 WorkingSet and private commit sizes, 1 sample ~1 second.
- Creates a deterministic ~5 MB Markdown fixture, opens via real useWorkspace/useTabs/Editor chain.
- Executes 120 CodeMirror editing transactions, 140 scroll frames and 10 close/reopen cycles.
- Reports CodeMirror dispatch p50/p95/p99, requestAnimationFrame turnaround p50/p95/p99 and RAM milestone samples.
- requestAnimationFrame turnaround is a latency proxy, not hardware key-to-pixel latency or precise FPS under continuous input.
- Sum of WorkingSet across processes double-counts shared memory pages. Treat it as a process-tree footprint upper estimate, not unique physical RAM.
- PrivateMemorySize64 is a private-commit metric, not physically resident RAM.

## Validity

The WebView test passes only if all app stages were received through a localhost telemetry endpoint and WebView2 child processes were observed. If hosted-runner desktop support is unavailable, the test fails as inconclusive (no invented result).

## Follow-up

Measure actual process-tree baseline before changing RAM policy. Later compare search indexing, CRDT snapshots, cache eviction, and hidden live tabs using equal workloads. No PR to Freaction until both performance and recovery guarantees are established.
