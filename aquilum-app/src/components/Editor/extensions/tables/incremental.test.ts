import { EditorState, Text } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import {
    findTablesAfterChanges,
    findTablesInDoc,
    type TableRange,
} from './constructs';
import { tablePreview } from './preview';
import { tableBoundaryExtension } from './boundary';

const TABLE = '| Name | Value |\n| --- | --- |\n| A | 1 |';
const TABLE_2 = '| X | Y |\n| --- | --- |\n| B | 2 |';

function independentScan(doc: Text): TableRange[] {
    // A fresh Text identity forces the original full scanner instead of cache.
    return findTablesInDoc(Text.of(doc.toString().split('\n')));
}

function checkChange(input: string, from: number, to: number, insert: string) {
    const state = EditorState.create({ doc: input });
    const before = findTablesInDoc(state.doc);
    const tr = state.update({ changes: { from, to, insert } });
    const updated = findTablesAfterChanges(tr.startState.doc, tr.newDoc, tr.changes);
    const expected = independentScan(tr.newDoc);
    expect(updated).toEqual(expected);
    expect(findTablesInDoc(tr.newDoc)).toBe(updated);
    return { before, updated, doc: tr.newDoc };
}

describe('incremental table range cache', () => {
    it('does not reparse the entire document after ordinary typing beyond all tables', () => {
        const source = ['begin', '', TABLE, '', ...Array(15).fill('ordinary text')].join('\n');
        const { before, updated } = checkChange(source, source.length, source.length, '!');
        expect(updated).toHaveLength(1);
        expect(updated[0]?.model).toBe(before[0]?.model);
    });

    it('adjusts table offsets before and between tables without rebuilding models', () => {
        const source = [
            ...Array(9).fill('preamble text'),
            TABLE,
            ...Array(9).fill('spacer text'),
            TABLE_2,
            ...Array(9).fill('trailing paragraph'),
        ].join('\n');
        const before = findTablesInDoc(EditorState.create({ doc: source }).doc);
        const first = checkChange(source, 0, 0, 'more preamble\n');
        expect(first.updated.map((x) => x.from)).toEqual(before.map((x) => x.from + 14));
        expect(first.updated[0]?.model).toBe(first.before[0]?.model);
        expect(first.updated[1]?.model).toBe(first.before[1]?.model);

        const middle = source.indexOf('spacer text') + 3;
        const result = checkChange(source, middle, middle, 'extra ');
        expect(result.updated[0]?.from).toBe(result.before[0]?.from);
        expect(result.updated[1]?.from).toBe(result.before[1]!.from + 6);
        expect(result.updated[1]?.model).toBe(result.before[1]?.model);
    });

    it('falls back to original parser when a complete table is created', () => {
        const headerOnly = 'lead\n| Col | Val |\n| --- | --- |\n';
        const result = checkChange(headerOnly, headerOnly.length, headerOnly.length, '| One | Two |');
        expect(result.before).toHaveLength(0);
        expect(result.updated).toHaveLength(1);
        expect(result.updated[0]?.model.cells[1]).toEqual(['One', 'Two']);
    });

    it('falls back and updates the widget model when editing inside a table', () => {
        const source = ['top', TABLE, '', 'bottom'].join('\n');
        const from = source.indexOf('| A | 1 |') + 6;
        const result = checkChange(source, from, from + 1, '99');
        expect(result.updated).toHaveLength(1);
        expect(result.updated[0]?.model).not.toBe(result.before[0]?.model);
        expect(result.updated[0]?.model.cells[1][1]).toBe('99');
    });

    it('recognizes table removal and newline joins', () => {
        const source = ['intro', TABLE, '', 'outro'].join('\n');
        const from = source.indexOf('| Name');
        const removal = checkChange(source, from, from + TABLE.length, 'ordinary paragraph');
        expect(removal.updated).toHaveLength(0);
        const broken = 'outside\n| Name | Value |\n| --- | --- |\n\n| A | 1 |\nfooter';
        const point = broken.indexOf('\n\n') + 1;
        const join = checkChange(broken, point, point + 1, '');
        expect(join.updated).toHaveLength(1);
    });

    it('preserves adjacent tables and metadata after distant changes', () => {
        const text = [
            TABLE,
            TABLE_2,
            '<!--q-table:{"cols":[115,140]}-->',
            '',
            ...Array(8).fill('ordinary text'),
        ].join('\n');
        const initial = independentScan(EditorState.create({ doc: text }).doc);
        const outcome = checkChange(text, text.length, text.length, '\npostscript');
        expect(outcome.updated).toEqual(initial);
        expect(outcome.updated).toHaveLength(2);
    });

    it('agrees with a fresh parser through deterministic mixed edits and reverse changes', () => {
        let seed = 0x98af3d2;
        const next = () => {
            seed ^= seed << 13;
            seed ^= seed >>> 17;
            seed ^= seed << 5;
            return seed >>> 0;
        };
        let state = EditorState.create({
            doc: ['prefix', TABLE, ...Array(7).fill('regular line'), TABLE_2, ...Array(15).fill('tail')].join('\n'),
        });
        findTablesInDoc(state.doc);
        const inserts = ['x', 'abc', '\n', '|', '<!--q-table:{}-->', 'Русский', ''];
        for (let i = 0; i < 170; i++) {
            const from = next() % (state.doc.length + 1);
            const length = next() % 4;
            const to = Math.min(state.doc.length, from + length);
            const insert = inserts[next() % inserts.length]!;
            const transaction = state.update({ changes: { from, to, insert } });
            const updated = findTablesAfterChanges(transaction.startState.doc, transaction.newDoc, transaction.changes);
            expect(updated).toEqual(independentScan(transaction.newDoc));

            const inverse = transaction.changes.invert(state.doc);
            const restored = transaction.state.update({ changes: inverse });
            const again = findTablesAfterChanges(restored.startState.doc, restored.newDoc, restored.changes);
            expect(again).toEqual(independentScan(restored.newDoc));
            state = transaction.state;
        }
    });

    it('retains actual table decorations and blank-line boundary behavior', () => {
        const source = ['intro', '', TABLE, '', ...Array(15).fill('ordinary line')].join('\n');
        let state = EditorState.create({
            doc: source,
            extensions: [tablePreview, tableBoundaryExtension],
        });
        const ranges = findTablesInDoc(state.doc);
        expect(ranges).toHaveLength(1);
        const oldFrom = ranges[0]!.from;
        state = state.update({ changes: { from: 0, insert: 'preface\n' } }).state;
        expect(findTablesInDoc(state.doc)[0]?.from).toBe(oldFrom + 8);
        // A real transaction filter must still insert the separating blank
        // line when a new table is created (and leave ordinary edits alone).
        const plain = EditorState.create({
            doc: 'Intro\n',
            extensions: [tablePreview, tableBoundaryExtension],
        });
        const added = plain.update({
            changes: { from: plain.doc.length, insert: TABLE },
        }).state;
        expect(findTablesInDoc(added.doc)).toHaveLength(1);
        expect(added.doc.toString().endsWith('\n')).toBe(true);
    });
});
