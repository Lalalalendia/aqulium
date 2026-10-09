#!/usr/bin/env python3
"""Test-only installer for a conservative, feature-preserving quote scan fast path.

Keeps the original scanners unchanged when no StateField hint is registered.
"""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
here = Path(__file__).resolve().parent
base = root / "aquilum-app/src/components/Editor/extensions"
field = base / "quoteScanPresence.ts"
assert not field.exists(), "source unexpectedly already has quote hint"
field.write_bytes((here / "quoteScanPresence.ts").read_bytes())
(base / "quoteScanPresence.test.ts").write_bytes((here / "quoteScanPresence.test.ts").read_bytes())

def patch(path, old, new):
    text = path.read_text(encoding="utf-8")
    assert text.count(old) == 1, f"upstream signature changed: {path} / {old}"
    path.write_text(text.replace(old, new), encoding="utf-8")

book = base / "bookCallout/constructs.ts"
patch(
    book,
    "import type { Text } from '@codemirror/state';",
    "import type { Text } from '@codemirror/state';\nimport { quotePresenceFor } from '../quoteScanPresence';",
)
patch(
    book,
    "export function findBookCallouts(doc: Text): BookCalloutSpan[] {",
    "export function findBookCallouts(doc: Text): BookCalloutSpan[] {\n  if (quotePresenceFor(doc)?.book === false) return [];",
)
reader = base / "readerQuote/constructs.ts"
patch(
    reader,
    "import type { Text } from '@codemirror/state';",
    "import type { Text } from '@codemirror/state';\nimport { quotePresenceFor } from '../quoteScanPresence';",
)
patch(
    reader,
    "export function findReaderQuotes(doc: Text): ReaderQuoteSpan[] {",
    "export function findReaderQuotes(doc: Text): ReaderQuoteSpan[] {\n  if (quotePresenceFor(doc)?.reader === false) return [];",
)
index = base / "index.ts"
patch(
    index,
    "import { livePreviewExtension } from './livePreviewPlugin';",
    "import { livePreviewExtension } from './livePreviewPlugin';\nimport { quoteScanPresence } from './quoteScanPresence';",
)
patch(
    index,
    "            bodySetup,",
    """            // Baseline keeps original scanner semantics; fastpath tracks missing
            // quote constructs conservatively and scans only affected lines.
            ...(typeof window !== 'undefined'
              && window.localStorage.getItem('aquilum_editor_ab_mode') === 'fastpath'
              ? [quoteScanPresence] : []),
            bodySetup,""",
)

# The mode must be set synchronously before React mounts (ephemeral runner only).
main = root / "aquilum-app/src/main.tsx"
s = main.read_text(encoding="utf-8")
needle = 'ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render('
assert s.count(needle) == 1, "unexpected renderer bootstrap"
boot = """if (import.meta.env.VITE_AQUILUM_RAM_LAB) {
  const request = new XMLHttpRequest();
  request.open('GET', 'http://127.0.0.1:18713/mode', false);
  request.send();
  if (request.status !== 200) throw new Error('RAM A/B server did not set the mode');
  window.localStorage.setItem('aquilum_editor_ab_mode', request.responseText);
}
"""
main.write_text(s.replace(needle, boot + '\n' + needle), encoding="utf-8")
ramLab = root / 'aquilum-app/src/modules/perf/ramLab.ts'
assert ramLab.exists(), "run main RAM patch before quote fastpath installer"
ramLab.write_bytes((here / 'ramLab.ts').read_bytes())
print("Installed conservative quote presence fastpath and baseline control; upstream remains untouched")
