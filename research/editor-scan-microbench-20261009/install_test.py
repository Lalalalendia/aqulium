#!/usr/bin/env python3
from pathlib import Path
import sys
root=Path(sys.argv[1]).resolve()
dst=root/"aquilum-app/src/components/Editor/extensions/editorScans.lab.test.ts"
assert dst.parent.is_dir() and not dst.exists()
dst.write_bytes(Path(__file__).with_name("editorScans.lab.test.ts").read_bytes())
print("Installed isolated frontend-only full-document scan performance test")
