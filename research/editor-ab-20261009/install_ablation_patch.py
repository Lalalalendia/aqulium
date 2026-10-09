#!/usr/bin/env python3
"""Temporary diagnostic toggles only; NOT proposed functional changes to upstream."""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
here = Path(__file__).resolve().parent
key = 'aquilum_editor_ab_mode'
mode_expr = f"(typeof window !== 'undefined' ? window.localStorage.getItem('{key}') : null)"

# Filter the renumber plugin at React editor construction time, not on ESM
# module evaluation. The runner supplies the mode at startup over localhost.
index = root / 'aquilum-app/src/components/Editor/extensions/index.ts'
s = index.read_text(encoding='utf-8')
import_anchor = "import { listCalloutsExtension, outlineExtension } from './outline';"
assert s.count(import_anchor) == 1, "unexpected extension imports"
s = s.replace(import_anchor, import_anchor + "\nimport { outlineRenumbering } from './outline/renumber';")
anchor = '            outlineExtension,\n'
assert s.count(anchor) == 1, "unexpected extension setup"
s = s.replace(anchor, (
    f"            ({mode_expr} === 'no_renumber' || {mode_expr} === 'both')"
    f" ? outlineExtension.filter((extension) => extension !== outlineRenumbering)"
    f" : outlineExtension,\n"
))
index.write_text(s, encoding='utf-8')

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
# The runner-controlled mode must be installed BEFORE React renders editor
# components; persisted WebView2 localStorage was not reliable in Actions.
bootstrap = root / 'aquilum-app/src/main.tsx'
s = bootstrap.read_text(encoding='utf-8')
bootstrap_anchor = 'ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render('
assert s.count(bootstrap_anchor) == 1, "unexpected renderer bootstrap"
bootstrap_code = """
if (import.meta.env.VITE_AQUILUM_RAM_LAB) {
  const request = new XMLHttpRequest();
  request.open('GET', 'http://127.0.0.1:18713/mode', false);
  request.send();
  if (request.status !== 200) throw new Error('RAM A/B server did not set the mode');
  window.localStorage.setItem('aquilum_editor_ab_mode', request.responseText);
}
"""
bootstrap.write_text(s.replace(bootstrap_anchor, bootstrap_code + '\n' + bootstrap_anchor), encoding='utf-8')
print("A/B diagnostic toggles installed; localhost-controlled per-process mode")
