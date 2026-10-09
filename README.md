# Aquilum: independent optimized version

Working downstream copy of [Freaction/Aquilum](https://github.com/Freaction/Aquilum), pinned to upstream commit a2ac434ba70c189a5e96e8712ec781ca608ad15c (v0.2.6). Not an official Aquilum release.

The full editor source, including UI, Rust core, Tauri, tests and assets, is in [aquilum-app/](aquilum-app/).

## Integrated improvements

- CodeMirror book and reader quote presence tracking avoids unnecessary full-document scans when editing ordinary text, without disabling quote functionality.
- Rust sparse Unicode offset mapping reduces transient memory during search.

## Development

In aquilum-app/: npm ci, npm test, npm run build.
Rust core: cargo test --manifest-path aquilum-app/core/Cargo.toml -p aquilum-core.

## Provenance and license

Original copyright remains with the Aquilum contributors. Original AGPL-3.0-only license applies; see [LICENSE](LICENSE). Foliate JS is materialized as regular source files from its upstream submodule checkout; exact upstream and submodule SHAs are recorded in [integration/UPSTREAM.lock](integration/UPSTREAM.lock).

Benchmark results and experimental scripts remain in research/ and integration/, with additional experiment branches. The rar2 repository is not involved.
