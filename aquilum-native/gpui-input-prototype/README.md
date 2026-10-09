# Native GPUI multiline editor (separate from the paged viewer)

This experiment adds a second, **editable** GPUI frontend alongside the
already built Iced multiline editor and the earlier GPUI paged viewer.

## Architecture

- `gpui = 0.2.2`, `gpui-component = 0.4.0-preview2` provide a
  persistent multiline InputState per tab. This provides native text editing
  operations without implementing a custom IME or text renderer.
- `aquilum-native-common` opens the **real** `aquilum-core::DocumentHub`.
- On explicit Save, the current GPUI buffer is submitted via
  `NativeDocument::save` using the version-checked replace_text path.
  An outdated version cannot silently overwrite another session's edits.
- A per-tab change listener marks the buffer dirty. Reload is rejected
  whenever unsaved changes are present.
- Multiple tabs retain their editor states after switching; no Tauri,
  browser engine or JSON IPC runs.

## Run (experimental; copies of notes only)

    cargo run --manifest-path aquilum-native/Cargo.toml -p aquilum-native-gpui-input --release -- --file C:\Temp\copy.md

Run the same note under Iced for a rough feature comparison:

    cargo run --manifest-path aquilum-native/Cargo.toml -p aquilum-native-iced --release -- --file C:\Temp\copy.md

Do not run separate processes against the same working document/store at the
same time; the CI lab uses copies. The native profile is separate from the
production Aquilum profile. Changes to supplied files occur only after Save.

## Verification gates

- Windows release build of the actual GPUI editor: GitHub Actions
  `aquilum-native-gpui-input.yml`. A successful compiler build does not
  prove the window accepts real keyboard/IME input. That requires a separate
  UI interaction test on Windows.
- Common core tests now include UTF-16/UTF-8 offset conversion, including
  emoji surrogate pairs, Cyrillic, combining marks and invalid positions.
- Fair RAM comparison uses GPUI **multiline Input**, not the old paged GPUI
  viewer, versus Iced's native text editor. Compare private bytes, process
  working set, and one/three-tab state after a stable window is observed.
- Both framework text editors still differ from CodeMirror in behavior:
  Markdown live preview, rich tables, chapter controls, multi-cursor, UI
  transaction undo integration, EPUB rendering and accessiblity need parity
  tests before selecting a replacement.

We are NOT measuring full keyboard-to-pixel latency or claiming savings
in WebView2 until real UI benchmarks and production parity are demonstrated.
This experiment does not modify the existing Aquilum Tauri UI or rar2.
