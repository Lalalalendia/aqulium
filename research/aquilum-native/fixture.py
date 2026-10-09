#!/usr/bin/env python3
"""Deterministic UTF-8 Markdown corpus for native GUI RAM smoke tests.

Creates 1/5/20 MiB notes; never touches a real user's file.
"""
from pathlib import Path
import json
import shutil
import sys

folder = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("native-fixtures")
folder.mkdir(parents=True, exist_ok=True)
parts = [
    "# Aquilum Native benchmark\n",
    "Русский текст, emoji 🌍, combining e\u0301 and 🇷🇺. Multilingual Markdown.\n",
    "- List one\n- List two\n",
    "| Name | Meaning |\n| --- | --- |\n| Номер | One |\n\n",
    "> [!quote] Источник [1](aquilum-reader:cfi=abc)\n",
    "See [link](next.md), [[note]], **bold** and *italic*.\n",
    "\n",
]
chunk = "".join(parts).encode("utf-8")
manifest = []
for mb in [1, 5, 20]:
    target = mb * 1024 * 1024
    file = folder / ("note-%02dmb.md" % mb)
    with file.open("wb") as stream:
        blocks = target // len(chunk)
        for _ in range(blocks):
            stream.write(chunk)
        stream.write(b"x" * (target - stream.tell()))
    raw = file.read_bytes()
    assert len(raw) == target
    raw.decode("utf-8")
    manifest.append({"mib": mb, "path": str(file.resolve()), "bytes": len(raw)})
for i in [2, 3]:
    other = folder / ("note-05mb-copy-%d.md" % i)
    shutil.copy2(folder / "note-05mb.md", other)
    manifest.append({"mib": 5, "path": str(other.resolve()), "bytes": other.stat().st_size})
(folder / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
print("AQUILUM_NATIVE_FIXTURES " + json.dumps(manifest, ensure_ascii=False))
