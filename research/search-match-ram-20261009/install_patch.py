#!/usr/bin/env python3
"""Add an isolated benchmark child module in the disposable pinned checkout."""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
target = root / "aquilum-app/core/src/search/matching.rs"
source = target.read_text(encoding="utf-8")
assert "struct NormalizedText" in source
assert "mod ram_lab;" not in source
source += '\n#[cfg(all(test, windows))]\n#[path = "matching_ram_lab.rs"]\nmod ram_lab;\n'
target.write_text(source, encoding="utf-8")
(target.parent / "matching_ram_lab.rs").write_bytes(
    (Path(__file__).parent / "matching_ram_lab.rs").read_bytes()
)
print("Installed isolated baseline/compact normalizer benchmark")
