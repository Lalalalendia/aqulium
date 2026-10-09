import { StateField, type Text } from '@codemirror/state';
import { isReaderQuoteHeader, parseReaderQuoteLine } from '../../../modules/docs/bookQuotes';
import { isBookCalloutHeader } from './blockquoteScan';

/**
 * Conservative presence hint for immutable CodeMirror documents.
 *
 * False means no matching construct exists; true means a construct may exist.
 * Once true, it stays true until editor reconstruction. This guarantees that
 * deleting a marker never hides later book/reader quote content accidentally.
 *
 * Unregistered Text values return undefined and use the original full scanner.
 */
export interface QuoteScanPresence {
  readonly book: boolean;
  readonly reader: boolean;
}

const hints = new WeakMap<Text, QuoteScanPresence>();
const NONE: QuoteScanPresence = { book: false, reader: false };

export function quotePresenceFor(doc: Text): QuoteScanPresence | undefined {
  return hints.get(doc);
}

function inspect(line: string, previous: QuoteScanPresence): QuoteScanPresence {
  if (previous.book && previous.reader) return previous;
  // Book/reader constructs always have a '>' at the start after whitespace.
  if (!line.trimStart().startsWith('>')) return previous;

  const book = previous.book || isBookCalloutHeader(line);
  const reader = previous.reader || isReaderQuoteHeader(line) || parseReaderQuoteLine(line) !== null;
  return book === previous.book && reader === previous.reader
    ? previous
    : { book, reader };
}

function initial(doc: Text): QuoteScanPresence {
  let presence = NONE;
  for (let line = 1; line <= doc.lines; line++) {
    presence = inspect(doc.line(line).text, presence);
    if (presence.book && presence.reader) break;
  }
  hints.set(doc, presence);
  return presence;
}

export const quoteScanPresence = StateField.define<QuoteScanPresence>({
  create(state) {
    return initial(state.doc);
  },
  update(previous, transaction) {
    if (!transaction.docChanged) return previous;
    const doc = transaction.state.doc;
    let current = previous;

    // A false presence flag can turn true only when the edit creates a
    // construct on an affected line. CodeMirror's changed ranges include
    // newline joins and boundary text edits.
    if (!current.book || !current.reader) {
      transaction.changes.iterChangedRanges((_fromA, _toA, fromB, toB) => {
        if (current.book && current.reader) return;
        const first = doc.lineAt(Math.min(fromB, doc.length)).number;
        const last = doc.lineAt(Math.min(toB, doc.length)).number;
        for (let line = first; line <= last; line++) {
          current = inspect(doc.line(line).text, current);
          if (current.book && current.reader) break;
        }
      });
    }

    hints.set(doc, current);
    return current;
  },
});
