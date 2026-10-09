import { readFileSync } from 'node:fs';
import { Text } from '@codemirror/state';
import { describe, it, expect } from 'vitest';
import { findTablesInDoc } from './tables/constructs';
import { findBookCallouts } from './bookCallout/constructs';
import { findReaderQuotes } from './readerQuote/constructs';
import { collectOrderedRenumberChanges } from './outline/renumber';

const path = process.env.AQUILUM_SCAN_FIXTURE;
if (!path) throw new Error('AQUILUM_SCAN_FIXTURE unset');

function median(values: number[]): number {
  const sorted = values.toSorted((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)]!;
}
function p95(values: number[]): number {
  const sorted = values.toSorted((a, b) => a - b);
  return sorted[Math.ceil(sorted.length * 0.95) - 1]!;
}

describe('Aquilum existing 5MB document scan profiles', () => {
  it('measures independent full document scanners in Node JS, not Windows UI', () => {
    const source = readFileSync(path, 'utf8');
    const doc = Text.of(source.split('\n'));
    expect(source.length).toBeGreaterThan(3_000_000);
    const cases = [
      { name: 'findBookCallouts', fn: () => findBookCallouts(doc) },
      { name: 'findReaderQuotes', fn: () => findReaderQuotes(doc) },
      { name: 'findTablesInDoc', fn: () => findTablesInDoc(doc) },
      { name: 'collectOrderedRenumberChanges', fn: () => collectOrderedRenumberChanges(doc) },
    ];
    console.log('AQUILUM_SCAN_FIXTURE chars=' + doc.length + ' lines=' + doc.lines);
    for (const { name, fn } of cases) {
      for (let i = 0; i < 3; i++) fn();
      const durations: number[] = [];
      let count = 0;
      for (let i = 0; i < 17; i++) {
        const begin = performance.now();
        const result = fn();
        durations.push(performance.now() - begin);
        count += result.length;
      }
      console.log('AQUILUM_SCAN_PROFILE name=' + name +
        ' n=17 p50_ms=' + median(durations).toFixed(3) +
        ' p95_ms=' + p95(durations).toFixed(3) +
        ' total_results=' + count);
      if (name === 'findTablesInDoc') expect(count).toBeGreaterThan(200);
    }
  }, 120_000);
});
