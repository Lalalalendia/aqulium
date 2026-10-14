#!/usr/bin/env python3
"""Import staged Rust models from a source snapshot ZIP; never authorize a WebView2 cutover.

Usage: python3 scripts/import-native-rust-stages.py snapshot.zip [--apply]
A clean checkout based on 2e5fb22 is required.
"""
import argparse
import pathlib
import re
import subprocess
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
BASE = "2e5fb22d3f8334078353172f53f121695ebdbf76"
PREFIX = "aqulium-" + BASE + "/"
NATIVE = ("aquilum-app/core/src/native_editor/", "aquilum-app/core/src/native_reader/",
          "aquilum-app/core/src/native_shell/", "aquilum-app/native-gpui/")
INTEGRATION = {
    "aquilum-app/core/src/lib.rs",
    "aquilum-app/core/src/documents/hub.rs",
    "aquilum-app/core/src/documents/session.rs",
    "aquilum-app/core/src/documents/changes.rs",
    "aquilum-app/core/Cargo.toml",
    "aquilum-app/src-tauri/Cargo.toml",
    ".github/workflows/validate-gpui-rope-editor-ab.yml",
    "research/gpui-rope-editor-stage2-3.md",
}


def check_checkout():
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip():
        raise RuntimeError("Working tree must be clean")
    subprocess.run(["git", "merge-base", "--is-ancestor", BASE, "HEAD"],
                   cwd=ROOT, check=True)


def fix_code(rel, text):
    if rel == "aquilum-app/core/src/native_reader/mod.rs":
        match = re.match(r"((?:pub (?:mod|use) library[^\n]*\n|\n)+)((?://![^\n]*\n)+)", text)
        if match:
            text = match.group(2) + "\n" + match.group(1) + text[match.end():]
    if rel == "aquilum-app/core/src/native_editor/tests.rs":
        # The uploaded benchmark println has unescaped JSON quotes. Do not merge bad Rust.
        bad = 'AQUILUM_GPUI_ROPE_BENCH {{"input_bytes":'
        if bad in text:
            # Escape JSON quotes for Rust's format string, preserving {{ and }}.
            lines = text.splitlines(keepends=True)
            for i, line in enumerate(lines):
                if bad in line:
                    lines[i] = line.replace('"input_bytes"', '\\"input_bytes\\"').replace(
                        '"rope_allocated_mib"', '\\"rope_allocated_mib\\"').replace(
                        '"viewport_max_chars"', '\\"viewport_max_chars\\"').replace(
                        '"viewport_kib"', '\\"viewport_kib\\"').replace(
                        '"open_ms"', '\\"open_ms\\"').replace(
                        '"dispatch_p50_ms"', '\\"dispatch_p50_ms\\"').replace(
                        '"dispatch_p95_ms"', '\\"dispatch_p95_ms\\"').replace(
                        '"ws_before_mib"', '\\"ws_before_mib\\"').replace(
                        '"ws_after_mib"', '\\"ws_after_mib\\"').replace(
                        '"priv_before_mib"', '\\"priv_before_mib\\"').replace(
                        '"priv_after_mib"', '\\"priv_after_mib\\"')
            text = "".join(lines)
    if rel == "aquilum-app/core/src/native_shell/cutover.rs":
        marker = "let ready_to_remove_webview2 = stages.iter().all(|s| s.passed);"
        if marker not in text:
            raise RuntimeError("Unknown cutover implementation; review manually")
        text = text.replace(marker, "let ready_to_remove_webview2 = false; // UI, IME and EPUB parity not validated")
    return text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=pathlib.Path)
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    check_checkout()
    changes = []
    with zipfile.ZipFile(args.archive) as src:
        for entry in src.infolist():
            if entry.is_dir() or not entry.filename.startswith(PREFIX):
                continue
            rel = entry.filename[len(PREFIX):]
            p = pathlib.PurePosixPath(rel)
            if not rel or rel.startswith("/") or ".." in p.parts or "\\" in rel:
                raise RuntimeError("Unsafe ZIP path")
            if rel not in INTEGRATION and not rel.startswith(NATIVE):
                continue
            data = fix_code(rel, src.read(entry).decode("utf-8")).encode("utf-8")
            target = ROOT / rel
            if not target.exists() or target.read_bytes() != data:
                changes.append((target, data))
    for dest, data in changes:
        print(dest.relative_to(ROOT), len(data), "bytes")
    print("Changed:", len(changes), "| dry run:", not args.apply)
    if args.apply:
        for dest, data in changes:
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes(data)
        print("Review diff, run cargo test and do not merge without GUI validation.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError, zipfile.BadZipFile) as exc:
        print("ERROR:", exc, file=sys.stderr)
        sys.exit(1)
