#!/usr/bin/env python3
"""Patch only the disposable Aquilum runner checkout, never the upstream repo."""
from pathlib import Path
import sys

repo = Path(sys.argv[1]).resolve()
root = repo / "aquilum-app" / "core" / "src" / "documents"
hub = root / "hub.rs"
mod = root / "mod.rs"
assert hub.is_file() and mod.is_file(), "Unexpected Aquilum structure"
text = hub.read_text(encoding="utf-8")
old = "        self.compact(&key, &session);\n        if session.dirty() {"
new = """        if !bench_skip_enabled() || blobs.len() != 1 || session.version() != 0 || session.dirty() {
            self.compact(&key, &session);
        }
        if session.dirty() {"""
assert text.count(old) == 1, "Open hot-path changed in upstream"
text = text.replace(old, new)
old = "        self.write_entry(core, &key, &mut entry);\n        self.compact(&key, &entry.session);"
new = """        self.write_entry(core, &key, &mut entry);
        if !bench_skip_enabled() || entry.session.version() != 0 {
            self.compact(&key, &entry.session);
        }"""
assert text.count(old) == 1, "Release hot-path changed in upstream"
text = text.replace(old, new)
text += """
// This runtime switch exists only in the disposable benchmark checkout.
fn bench_skip_enabled() -> bool {
    std::env::var_os("AQUILUM_BENCH_SKIP_CLEAN_COMPACT").is_some()
}
"""
hub.write_text(text, encoding="utf-8")
module = mod.read_text(encoding="utf-8")
assert "native_perf_lab" not in module
mod.write_text(module + "\n#[cfg(test)]\nmod native_perf_lab;\n", encoding="utf-8")
source = Path(__file__).with_name("native_perf_lab.rs")
(root / source.name).write_bytes(source.read_bytes())
print("Patched pinned Aquilum DocumentHub for A/B native bench.")
