#!/usr/bin/env python3
"""Temporary diagnostic toggles only; NOT proposed functional changes to upstream."""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
here = Path(__file__).resolve().parent
key = 'aquilum_editor_ab_mode'
mode_expr = f"(typeof window !== 'undefined' ? window.localStorage.getItem('{key}') : null)"

outline = root / 'aquilum-app/src/components/Editor/extensions/outline/index.ts'
s = outline.read_text(encoding='utf-8')
anchor = '    outlineRenumbering,\n'
assert s.count(anchor) == 1, "unexpected outline setup"
s = s.replace(anchor, (
    f"    ({mode_expr} === 'no_renumber' || {mode_expr} === 'both') ? [] : outlineRenumbering,\n"
))
outline.write_text(s, encoding='utf-8')

for relative, definition in [
    ('bookCallout/constructs.ts', 'export function findBookCallouts(doc: Text): BookCalloutSpan[] {'),
    ('readerQuote/constructs.ts', 'export function findReaderQuotes(doc: Text): ReaderQuoteSpan[] {'),
]:
    target = root / 'aquilum-app/src/components/Editor/extensions' / relative
    s = target.read_text(encoding='utf-8')
    assert s.count(definition) == 1, f"unexpected scanner signature: {relative}"
    s = s.replace(definition, definition + "\n" +
        f"  if ({mode_expr} === 'no_fullscans' || {mode_expr} === 'both') return [];\n")
    target.write_text(s, encoding='utf-8')

frontend = root / 'aquilum-app/src/modules/perf/ramLab.ts'
assert frontend.exists(), "run install_ram_patch.py desktop before installing A/B"
frontend.write_bytes((here / 'ramLab.ts').read_bytes())
print("A/B diagnostic toggles installed; no source change committed upstream")
