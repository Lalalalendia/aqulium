# Aquilum Native Desktop (experimental)

This is a **separate eframe/egui OS-window executable** for the existing
Rust `GpuiWorkspaceWindow` controller and `GpuiSceneGraph`. It is **not**
the original Zed GPUI library, a replacement for Tauri, or a release build.

The `[workspace]` declaration deliberately isolates its GUI dependencies
from the production `src-tauri/Cargo.lock`. This is necessary until real
GUI tests, platform packaging, and upstream compatibility are resolved.

## Run

```sh
cargo run --manifest-path aquilum-app/native-desktop/Cargo.toml -- /absolute/path/to/vault
cargo run --manifest-path aquilum-app/native-desktop/Cargo.toml -- /absolute/path/to/note.md --app-data /tmp/aquilum-native-state
```

The application uses the existing Rust core, text editor, and scene graph.
It draws real OS window frames through eframe and routes pointer, keyboard,
text, paste and IME events to Rust controllers.

### Implemented initial interactions

- Navigate by sidebar file hitboxes and document tabs.
- Edit Markdown via physical keyboard, Unicode text input and IME composition.
- Undo/redo, cursor movement, select-all and explicit save (Ctrl/Cmd+S).
- Approximate line/column pointer placement, scroll and drag selection.
- Window exit calls the native workspace shutdown routine.

### Known limitations / release gate

- **Do not remove WebView2/Tauri.** No production GUI parity is established.
- Pointer column position uses approximate glyph advance, not font shaping.
- Rich editor overlays, settings, graph interactions, reader pagination,
  drag/drop, accessibility and platform-native clipboard parity are incomplete.
- CI can prove Windows/Linux compilation but cannot establish that an
  actual user typed and saved a note via a visible window or system IME.
- The prototype has not been tested against the full real-world EPUB corpus.
- Cargo.lock for the isolated prototype is not yet version-pinned. CI's cargo
  dependency resolution and OS compilation are preliminary, not a reproducible release.

Do not use this experimental executable with irreplaceable data.
Use a copy of a workspace when testing.
