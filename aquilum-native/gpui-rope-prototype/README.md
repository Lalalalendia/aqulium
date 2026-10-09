# Bounded GPUI Rope editor (laboratory prototype)

Each open tab holds one Rope and sends only 4096 Unicode scalar values to its GPUI Input widget. Buttons navigate fixed character windows, including documents with extremely long single lines.

Save computes a single Unicode-safe contiguous edit from a bounded widget snapshot, sends CodeMirror-compatible UTF-16 ChangeSet JSON through Aquilum's existing DocumentHub/Yrs with version checks, and updates the local Rope only after server acceptance. It does not send full document text on Save. Large pasted windows (>8192 Unicode scalars) are rejected and retain unsaved text. Pasted CRLF is normalized to LF.

Aquilum's core still stores the full document, and GPUI Input state may incur separate allocations. Headless Rope memory figures are NOT equivalent to total process memory. This is not production-ready: automated IME/selection/Undo/Redo, visual scrolling, crash recovery, dirty-exit prompt and actual typing latency are not verified. Character windows can break grapheme clusters.

Only open disposable Markdown files; Save modifies those selected files.

Windows:

    cargo run --manifest-path aquilum-native/Cargo.toml -p aquilum-native-gpui-rope --release -- --file C:\Temp\note.md

Tests:

    cargo test --manifest-path aquilum-native/Cargo.toml -p aquilum-native-rope-viewport -p aquilum-native-common -- --test-threads=1
