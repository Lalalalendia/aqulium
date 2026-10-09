import { EditorState } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { findBookCallouts } from './bookCallout/constructs';
import { findReaderQuotes } from './readerQuote/constructs';
import { quotePresenceFor, quoteScanPresence } from './quoteScanPresence';

const create = (text: string) => EditorState.create({ doc: text, extensions: [quoteScanPresence] });
const change = (state: EditorState, from: number, to: number, insert: string) =>
  state.update({ changes: { from, to, insert } }).state;

describe('incremental quote scan presence', () => {
  it('skips absent constructs while keeping unchanged document behavior', () => {
    let state = create('First paragraph\nПростой текст\nNo special markup');
    expect(quotePresenceFor(state.doc)).toEqual({ book: false, reader: false });
    expect(findBookCallouts(state.doc)).toEqual([]);
    expect(findReaderQuotes(state.doc)).toEqual([]);

    state = change(state, state.doc.length, state.doc.length, '\nAnother paragraph');
    expect(quotePresenceFor(state.doc)).toEqual({ book: false, reader: false });
    expect(findBookCallouts(state.doc)).toEqual([]);
    expect(findReaderQuotes(state.doc)).toEqual([]);
  });

  it('detects a book marker typed one character at a time', () => {
    let state = create('Introduction\n');
    for (const character of '> [!book] [[Книга]]') {
      state = change(state, state.doc.length, state.doc.length, character);
    }
    expect(quotePresenceFor(state.doc)?.book).toBe(true);
    expect(findBookCallouts(state.doc)).toHaveLength(1);
  });

  it('detects a reader header and a legacy reader link', () => {
    const header = '> [!quote] Текст [1](aquilum-reader:cfi=abc)';
    let state = create('Introduction\n');
    state = change(state, state.doc.length, state.doc.length, header);
    expect(quotePresenceFor(state.doc)?.reader).toBe(true);
    expect(findReaderQuotes(state.doc)).toHaveLength(1);

    const legacy = '> Quoted text[[→]](aquilum-reader:cfi=xyz)';
    const other = create('Plain\n' + legacy);
    expect(quotePresenceFor(other.doc)?.reader).toBe(true);
    expect(findReaderQuotes(other.doc)).toHaveLength(1);
  });

  it('detects markers created by a newline deletion', () => {
    let state = create('> \n[!book] [[A]]');
    expect(quotePresenceFor(state.doc)?.book).toBe(false);
    const newline = state.doc.toString().indexOf('\n');
    state = change(state, newline, newline + 1, '');
    expect(quotePresenceFor(state.doc)?.book).toBe(true);
    expect(findBookCallouts(state.doc)).toHaveLength(1);
  });

  it('stays conservative after deletion and reparses correctly', () => {
    let state = create('First\n> [!book] [[A]]\nLast');
    expect(findBookCallouts(state.doc)).toHaveLength(1);
    const from = state.doc.toString().indexOf('> [!book]');
    state = change(state, from, from + '> [!book] [[A]]'.length, 'New paragraph');
    expect(quotePresenceFor(state.doc)?.book).toBe(true);
    expect(findBookCallouts(state.doc)).toEqual([]);
  });

  it('does not make standalone Text objects depend on the editor field', () => {
    const withoutField = EditorState.create({ doc: '> [!book] [[A]]' });
    expect(quotePresenceFor(withoutField.doc)).toBeUndefined();
    expect(findBookCallouts(withoutField.doc)).toHaveLength(1);
  });

  it('preserves mixed content even after unrelated edits', () => {
    let state = create('> [!book] [[A]]\nParagraph\n> [!quote] Quote [1](aquilum-reader:cfi=abc)');
    expect(quotePresenceFor(state.doc)).toEqual({ book: true, reader: true });
    expect(findBookCallouts(state.doc)).toHaveLength(1);
    expect(findReaderQuotes(state.doc)).toHaveLength(1);
    state = change(state, 0, 0, 'Preface\n');
    expect(findBookCallouts(state.doc)).toHaveLength(1);
    expect(findReaderQuotes(state.doc)).toHaveLength(1);
  });
});
