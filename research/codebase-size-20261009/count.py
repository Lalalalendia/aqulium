#!/usr/bin/env python3
"""Measure actual source LOC from the checked-out dedicated Aquilum repo."""
from __future__ import annotations
from collections import defaultdict
from pathlib import Path
import json
import re
import subprocess

repo = Path(__file__).resolve().parents[2]
skip = {".git", "node_modules", "target", "dist", "build", ".rescript", "coverage"}
suffixes = {".ts", ".tsx", ".rs", ".js"}
test_re = re.compile(r"(?:\.test|\.spec)\.[jt]sx?$")
entries = []
for path in repo.rglob("*"):
    if not path.is_file() or path.suffix not in suffixes:
        continue
    relative = path.relative_to(repo).as_posix()
    if any(part in skip for part in path.relative_to(repo).parts):
        continue
    raw = path.read_bytes()
    contents = raw.decode("utf-8-sig")
    lines = contents.splitlines()
    nonempty = sum(bool(line.strip()) for line in lines)
    comment_only = sum(
        bool(line.strip()) and (
            line.lstrip().startswith("//")
            or line.lstrip().startswith("/*")
            or line.lstrip().startswith("* ")
            or line.lstrip().startswith("*/")
        )
        for line in lines
    )
    is_test = bool(test_re.search(relative) or "/__tests__/" in relative)
    entries.append({
        "path": relative,
        "language": "typescript" if path.suffix in {".ts", ".tsx"} else (
            "rust" if path.suffix == ".rs" else "javascript"
        ),
        "extension": path.suffix,
        "is_test": is_test,
        "bytes": len(raw),
        "lines": len(lines),
        "nonblank": nonempty,
        "nonblank_noncomment_approx": nonempty - comment_only,
    })

def group(label, rows):
    totals = {
        "group": label, "files": len(rows),
        "bytes": sum(x["bytes"] for x in rows),
        "lines": sum(x["lines"] for x in rows),
        "nonblank": sum(x["nonblank"] for x in rows),
        "nonblank_noncomment_approx": sum(x["nonblank_noncomment_approx"] for x in rows),
    }
    print("AQUILUM_LOC " + json.dumps(totals, ensure_ascii=False))
    return totals

ts = [x for x in entries if x["language"] == "typescript"]
app = [x for x in ts if x["path"].startswith("aquilum-app/src/")]
app_prod = [x for x in app if not x["is_test"]]
app_test = [x for x in app if x["is_test"]]
rs = [x for x in entries if x["language"] == "rust"]
groups = [
    group("all TypeScript/TSX", ts),
    group("frontend app TS/TSX", app),
    group("frontend runtime TS/TSX", app_prod),
    group("frontend tests TS/TSX", app_test),
    group("frontend Editor TS/TSX", [x for x in app if x["path"].startswith("aquilum-app/src/components/Editor/")]),
    group("frontend Editor runtime TS/TSX", [x for x in app_prod if x["path"].startswith("aquilum-app/src/components/Editor/")]),
    group("frontend Graph TS/TSX", [x for x in app if x["path"].startswith("aquilum-app/src/components/Graph/")]),
    group("all Rust", rs),
    group("app Rust core", [x for x in rs if x["path"].startswith("aquilum-app/core/src/")]),
    group("all JS", [x for x in entries if x["language"] == "javascript"]),
]
byarea = defaultdict(list)
for x in app:
    parts = x["path"].split("/")
    key = "/".join(parts[:4])
    byarea[key].append(x)
areas = [
    {"area": key,
     "files": len(rows),
     "bytes": sum(x["bytes"] for x in rows),
     "lines": sum(x["lines"] for x in rows),
     "nonblank": sum(x["nonblank"] for x in rows),
     "tests": sum(x["is_test"] for x in rows),
    }
    for key, rows in byarea.items()
]
areas.sort(key=lambda x: x["lines"], reverse=True)
for area in areas[:15]:
    print("AQUILUM_LOC_AREA " + json.dumps(area, ensure_ascii=False))

sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
report = {
    "source_commit": sha,
    "definition": "Physical lines via Python splitlines; nonblank excludes whitespace-only lines; nonblank_noncomment_approx excludes single-line comment-only lines by simple prefix (not a full parser).",
    "groups": groups,
    "top_areas": areas,
}
output = repo / "research/codebase-size-20261009"
output.mkdir(parents=True, exist_ok=True)
(output / "counts.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("AQUILUM_LOC_SHA " + sha)
