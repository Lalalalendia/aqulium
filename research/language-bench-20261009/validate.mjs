import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { scanMarkers as typescript } from './dist/ts.mjs';
import { scanMarkers as rescript } from './dist/rescript.mjs';
import { fixtureOf } from './fixture.mjs';

const require = createRequire(import.meta.url);
const wasm = require('./rust/pkg/aquilum_scan_bench.js');
const rust = wasm.scan_markers;

function oracle(text) {
  let book = 0;
  let quote = 0;
  for (const line of text.split('\n')) {
    const match = /^[ \t]*>[ \t]*\[!(book|quote)\]/i.exec(line);
    if (match?.[1]?.toLowerCase() === 'book') book++;
    else if (match?.[1]?.toLowerCase() === 'quote') quote++;
  }
  return book * 65536 + quote;
}

const cases = [
  '',
  'Plain Markdown paragraph with no callout markers',
  '> [!book] [[Книга]]',
  '  > \t[!QUoTE] a citation',
  '> [!Book] mixed capitalization\n>\t[!qUotE] mixed\n',
  'The phrase > [!book] appears within regular paragraph text',
  '> [!quote] Cyrillic \r\n next',
  '\t\t>    [!book] wiki ref\n\n> [!quote] reader quote',
  '>notabook\n> [!notes] foo\n  > [!bookish] should not match',
  '\n  > \t[!BooK] line\nText > [!quote]\n > [!QuOtE] quote\n',
  'some emoji 😺 and 日本語\n  > [!quote] цитата',
  '> [!quote]\r\n> [!book]\r\nA\r\n',
];
for (const text of cases) {
  const expected = oracle(text);
  for (const [name, scan] of [['typescript', typescript], ['rescript', rescript], ['rustwasm', rust]]) {
    assert.equal(scan(text), expected, name + ' differs from independent regex oracle on ' + JSON.stringify(text));
  }
}

for (const [name, mb] of [['mixed', 1], ['mixed', 5], ['dense', 5], ['mixed', 20], ['ascii', 5]]) {
  const f = fixtureOf(name, mb);
  assert.equal(oracle(f.text), f.encodedResult, 'fixture expected counts ' + name + '/' + mb);
  for (const [variant, scan] of [['typescript', typescript], ['rescript', rescript], ['rustwasm', rust]]) {
    assert.equal(scan(f.text), f.encodedResult, variant + ' differs on ' + name + '/' + mb + 'MB');
  }
  console.log('AQUILUM_LANG_PARITY ' + JSON.stringify({
    profile: name, mb, utf8Bytes: f.utf8Bytes, units: f.utf16Units,
    book: f.books, quote: f.quotes,
  }));
}
console.log('AQUILUM_LANG_CORRECTNESS all three implementations match the independent oracle');
