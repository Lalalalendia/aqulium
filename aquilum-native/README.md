# Aquilum Native: Strangler Fig experiment

**Status: experimental, do not edit important notes.** This is an independent
native Windows UI investigation built within the existing Aquilum repository.
The TypeScript/Tauri product under aquilum-app/ remains unchanged.

## What is actually implemented

- common/: a native Rust adapter around the **existing aquilum-core::Core and
  DocumentHub**, rather than a parallel Markdown storage implementation.
  It opens real files, holds sessions, saves through DocumentHub with stale
  version checks, can refresh, and releases sessions on Drop. Uses a separate
  lab app-data directory and does not import the user's existing workspace.
- iced-prototype/: a native Iced 0.14 window with *real multiline text input*,
  multiple tabs, explicit Save and guarded Reload.
- gpui-prototype/: a GPUI window that loads the *same documents* from the
  same common adapter; currently a paged Markdown viewer with tab switching,
  append-test-line, and explicit Save. **Keyboard text editing is NOT yet
  implemented** and this prototype must not be compared with Iced as if they
  had feature parity. That is the next milestone.
- GitHub Actions checks both frontends on Windows and runs the common backend
  unit suite on Windows and Linux.

The UI integrations are isolated prototypes. They do not replace the Rust
backend, and they do not use Tauri, Preact, WebView2, JSON/IPC, or CodeMirror.

## Run (on a developer Windows computer)

Use copies of Markdown notes. Pass each path deliberately; no note is created
without user input.

    cargo run --manifest-path aquilum-native/Cargo.toml -p aquilum-native-iced --release -- --file C:\Temp\sample.md
    cargo run --manifest-path aquilum-native/Cargo.toml -p aquilum-native-gpui --release -- --file C:\Temp\sample.md

For multiple open tabs append --file C:\Temp\second.md etc.

Tests (work on Windows or Linux):

    cargo test --manifest-path aquilum-native/Cargo.toml -p aquilum-native-common -- --test-threads=1

The backend profile is an isolated temporary directory named by process ID.
Saving **does modify the explicitly opened Markdown file**. Unsaved editor
content is NOT automatically saved on exit; Iced currently has no confirmation
dialog. The GPUI appendix is a fixed example line and NOT real text input.

## Migration decision / parity gate

1. First prove common backend open/save/stale-version safety for Unicode and
   reload. Must not modify documents with uncommitted edits unintentionally.
2. Bring GPUI up to the same multiline editing, Undo/Redo, input method editor,
   selection and viewport rules as Iced. Until then, no apples-to-apples GUI
   benchmark.
3. Benchmark physical working set, private committed memory, true desktop
   startup, actual keyboard-to-pixel latency and scroll frame times on Windows
   on identical 1/5/20 MiB Markdown notes with 1/3 live tabs.
4. Compare with the existing Tauri baseline. Native GPU libraries have their
   own overhead; Rust source code alone does not guarantee lower GUI RAM.
5. Evolve a shared editing protocol using UTF-16 offsets for Yrs. Diff-based
   transactions, undo state and crash recovery must pass tests before
   replacing any production editing path.
6. Migrate the shell/search/files first; preserve the current CodeMirror and
   Foliate-powered EPUB reader as references until each has parity and is
   tested with real UI pointer and keyboard interactions.

No whole-app rewrite, automatic default switch or changes to the existing
main UI are part of this experiment.

## Maintained source and copyright

Original Aquilum source is AGPL-3.0-only, as is this experimental prototype.
GPUI is Apache-2.0 and Iced is MIT-licensed. Their respective dependency
licenses remain their own.

See [architecture.md](architecture.md) for the transition contract.
