#!/usr/bin/env python3
"""Apply proposed production quote scanning fastpath to exact original upstream checkout."""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
ext = root / "aquilum-app/src/components/Editor/extensions"
assert ext.is_dir()
here = Path(__file__).resolve().parent
destination = ext / "quoteScanPresence.ts"
assert not destination.exists(), "upstream already has quoteScanPresence"
destination.write_bytes((here / "quoteScanPresence.ts").read_bytes())
(ext / "quoteScanPresence.test.ts").write_bytes((here / "quoteScanPresence.test.ts").read_bytes())

def one(path, original, proposed):
    src = path.read_text(encoding="utf-8")
    assert src.count(original) == 1, f"cannot identify pinned upstream anchor: {path}"
    path.write_text(src.replace(original, proposed), encoding="utf-8")

book = ext / "bookCallout/constructs.ts"
one(book,
    "import type { Text } from '@codemirror/state';",
    "import type { Text } from '@codemirror/state';\nimport { quotePresenceFor } from '../quoteScanPresence';")
one(book,
    "export function findBookCallouts(doc: Text): BookCalloutSpan[] {",
    "export function findBookCallouts(doc: Text): BookCalloutSpan[] {\n  if (quotePresenceFor(doc)?.book === false) return [];")

reader = ext / "readerQuote/constructs.ts"
one(reader,
    "import type { Text } from '@codemirror/state';",
    "import type { Text } from '@codemirror/state';\nimport { quotePresenceFor } from '../quoteScanPresence';")
one(reader,
    "export function findReaderQuotes(doc: Text): ReaderQuoteSpan[] {",
    "export function findReaderQuotes(doc: Text): ReaderQuoteSpan[] {\n  if (quotePresenceFor(doc)?.reader === false) return [];")

index = ext / "index.ts"
one(index,
    "import { livePreviewExtension } from './livePreviewPlugin';",
    "import { livePreviewExtension } from './livePreviewPlugin';\nimport { quoteScanPresence } from './quoteScanPresence';")
one(index, "            bodySetup,", "            quoteScanPresence,\n            bodySetup,")
print("Applied production-shaped quote scanning fastpath without flags or mode endpoints")
