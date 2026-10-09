# Aquilum Native: GPUI + bounded Rope

**Experimental. Use copies of Markdown notes only.** The original Tauri /
CodeMirror product is unchanged.

## What works in source

- Multiple independent document tabs share the REAL Aquilum-core DocumentHub.
- Complete text is held by a ropey Rope, while GPUI Input sees at most 8192
  Unicode scalars at any moment, even if the file has one enormous line.
- Previous/Next page buttons navigate the document in small bounded windows.
- Switching tabs/pages with unsaved input is explicitly refused.
- Save delta derives a compact CodeMirror-compatible UTF-16 change from the
  currently edited window, submits it through the real version-checked
  DocumentHub/Yrs path, and commits Rope only after ACK.
- Reload is refused when there are unsaved text edits.
- An automated Windows runner compares its private RAM with the previous
  full-document GPUI Input on the SAME 1, 5, 20 MiB Markdown fixture files.

## Running on Windows with TEST COPIES

    cargo run --manifest-path aquilum-native/Cargo.toml -p aquilum-native-gpui-rope --release -- --file C:\Temp\note-copy.md

More tabs can be supplied via further --file PATH arguments.

## Serious outstanding constraints

- The GUI has NOT yet passed automated physical keyboard, mouse, IME or
  accessibility interactions. Successful compilation or a window handle is
  not equivalent to editor correctness.
- A DocumentHub push ACK does not prove an fsync or power-failure durability.
  Disk journal errors may be swallowed by the existing backend. Real Save,
  cross-process conflict behavior and crash recovery are still open gates.
- UI widget undo/redo is not linked to persistent Yrs history. Several
  disjoint edits in a viewport are collapsed into one bounded replacement,
  preserving text but possibly changing collaborative undo semantics.
- No Markdown rich preview, EPUB, graph or full CodeMirror extension parity.
- The program does NOT confirm before closing with unsaved edits. Test files
  only, never user data or a synced repository.
- Aquilum already normalizes CRLF/lone CR to LF in its Rust file reader.
  All position calculations must use DocumentHub's canonical text.
- The backend Yrs replica and the Rope both own document state; a headless
  Rope model with low RAM is not evidence of equally low whole-app RAM.

## Previously verified same-host Windows baseline

[Headless Iced vs Rope run #37981178229](https://github.com/Lalalalendia/aqulium/actions/runs/37981178229)

| Document | Iced private MiB | Rope private MiB |
| --- | ---: | ---: |
| 1 MiB | 328.88 | 1.98 |
| 5 MiB | 1615.46 | 6.68 |
| 20 MiB | exceeded 2.3 GiB | 24.51 |

These are headless model processes. Compare FULL GUI processes with
the new Windows workflow aquilum-native-gpui-rope.yml before selecting a
framework. Earlier production WebView2 runs were near 203-238 MiB private
commit for three 5-MiB tabs but differed in feature set and fixtures.

Release requires physical UI interaction tests, per-edit safe undo,
durability and full Markdown parity. Keep working TypeScript as fallback.
