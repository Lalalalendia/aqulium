import { EditorState } from '@codemirror/state';
import { describe, expect, it } from 'vitest';
import { findTablesInDoc } from './constructs';
import { tablePreview } from './preview';
import { tableBoundaryExtension } from './boundary';

function fixture(): string {
    const lines: string[] = ['# Large Markdown notebook', ''];
    let bytes = 0;
    for (let i = 0; bytes < 5_000_000; i++) {
        let line: string;
        if (i % 160 === 0) {
            line = '\n| Heading | Value |\n| --- | --- |\n| Unit | ' + i + ' |\n';
        } else {
            line = 'Ordinary note ' + i + ': Russian and English paragraphs with prose, ' +
                'links [text](local.md), references [[local]], formatting **bold**, ' +
                'many words but no table delimiters. The reader is typing at the end.';
        }
        lines.push(line);
        bytes += line.length + 1;
    }
    return lines.join('\n');
}

function median(data: number[]) {
    const sorted = data.toSorted((a, b) => a - b);
    return sorted[Math.floor(sorted.length / 2)]!;
}
function percentile(data: number[], portion: number) {
    const sorted = data.toSorted((a, b) => a - b);
    return sorted[Math.ceil(sorted.length * portion) - 1]!;
}

describe('5MB real CodeMirror State table transaction profile', () => {
    it('measures 120 typed changes including boundary filters and preview, retaining table semantics', () => {
        const doc = fixture();
        let state = EditorState.create({
            doc,
            extensions: [tablePreview, tableBoundaryExtension],
        });
        const originalTables = findTablesInDoc(state.doc);
        expect(state.doc.length).toBeGreaterThan(4_000_000);
        expect(originalTables.length).toBeGreaterThan(100);
        const timings: number[] = [];
        const allTables = originalTables.length;
        // Include multiple changes but exclude 8 JIT warmups from percentile stats.
        for (let i = 0; i < 128; i++) {
            const start = performance.now();
            const pos = state.doc.length;
            state = state.update({
                changes: { from: pos, insert: 'x' },
                selection: { anchor: pos + 1 },
            }).state;
            const ms = performance.now() - start;
            if (i >= 8) timings.push(ms);
        }
        const endTables = findTablesInDoc(state.doc);
        expect(endTables.length).toBe(allTables);
        expect(endTables[0]?.from).toBe(originalTables[0]?.from);
        expect(endTables[allTables - 1]?.from).toBe(originalTables[allTables - 1]?.from);
        expect(state.doc.sliceString(state.doc.length - 128)).toBe('x'.repeat(128));

        const results = {
            variant: process.env.AQUILUM_TABLE_VARIANT || 'unspecified',
            characters: state.doc.length,
            table_count: allTables,
            sample_count: timings.length,
            p50_ms: median(timings),
            p95_ms: percentile(timings, 0.95),
            p99_ms: percentile(timings, 0.99),
            max_ms: Math.max(...timings),
            heap_mib: process.memoryUsage().heapUsed / (1024 * 1024),
            rss_mib: process.memoryUsage().rss / (1024 * 1024),
        };
        console.log('AQUILUM_TABLE_BENCH ' + JSON.stringify(results));
    }, 180_000);
});
