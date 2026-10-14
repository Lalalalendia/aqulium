# Experimental native Rust modules: integration notes

Source: user-supplied `aqulium-native-rust-optimized-v2.zip` (base `2e5fb22d3f8334078353172f53f121695ebdbf76`).

This commit adds experimental Rust **models** for a Rope-backed editor, workspace, Markdown interactions, search, graph scene model and FB2 reading. It does **not** provide a platform-native GPUI renderer; `native-gpui` is currently a headless model/CLI despite its historical name. Zipped `.epub` parsing is not implemented and opening such files fails explicitly instead of reading ZIP bytes as UTF-8.

The existing Tauri/WebView2 app and its production data flow remain canonical. The stage-7 smoke gate cannot approve WebView2 removal. Memory/speed numbers in uploaded research are unverified here; GUI parity, actual keystroke latency, IME/accessibility, save recovery and physical EPUB rendering still require proof.

## Verification

- Run: `cargo test --manifest-path aquilum-app/src-tauri/Cargo.toml -p aquilum-core native_ -- --test-threads=1`
- Run: `cargo test --manifest-path aquilum-app/src-tauri/Cargo.toml -p aquilum-native-gpui -- --test-threads=1`
- Run: `cargo check --manifest-path aquilum-app/src-tauri/Cargo.toml -p aquilum-native-gpui`
- Do not use `--verify-cutover` as a successful production-readiness assertion. A nonzero cutover exit means insufficient proof, by design.

## Integration checkpoint

- Source snapshot: `aqulium-native-rust-optimized-v2.zip` (same base as `main`).
- Native source models are checked in directly on a separate GitHub feature branch.
- `aquilum-app/Cargo.toml` from the ZIP is intentionally excluded because the pre-existing
  Tauri `src-tauri/Cargo.toml` is the canonical Cargo workspace. The new crate is
  enrolled in that workspace via `../native-gpui`.
- Calling the CLI binary must not be mistaken for a real OS window; it prints view-state only.
- Unicode PDF inputs now return errors instead of lossy transliteration.
- The EPUB extension is rejected with a specific error until ZIP/OPF/spine is implemented.
- The native model CI checks Rust compilation and smoke. It does **not** test
  keyboard-to-pixel latency, real selection/IME input, accessibility or GUI parity.
