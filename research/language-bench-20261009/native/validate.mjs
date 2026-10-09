import assert from 'node:assert/strict';
import { writeFileSync, mkdirSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { scanMarkers as typescript } from '../dist/ts.mjs';

const require = createRequire(import.meta.url);
const wasm = require('../rust/pkg/aquilum_scan_bench.js');
const expectedFromRegex = text => (
  ((text.match(/^[ \t]*>[ \t]*\[!book\]/gim) || []).length * 65536) +
  (text.match(/^[ \t]*>[ \t]*\[!quote\]/gim) || []).length
);
const tests = [
  '', 'English plain text\n', 'Русская заметка без цитат\n',
  '> [!book] [[Книга]]\n', '  >\t[!QUOTE] Свидетельство\n',
  '\t>  [!BoOk] English and кириллица\n> [!QuOtE] Параграф',
  'Midline > [!book] should NOT count',
  ' > [!bookish] not a real book marker\n> [!quotes] no',
  '> [!book]\r\n> [!quote]\r\nnext\r\n',
  'some emoji 🌍\n\t  >[!book] recognized\n日本語 test\n',
  '>notquote\n>\t[!quote] Japanese 日本語\n\nPlain',
];
mkdirSync('native/tmp', { recursive: true });
for (const [i, text] of tests.entries()) {
  const expected = expectedFromRegex(text);
  const cached = new wasm.PreloadedScanner(text);
  assert.equal(cached.byte_len(), Buffer.byteLength(text), 'Wasm retained UTF8 byte count');
  assert.equal(typescript(text), expected, 'V8 parity case '+i);
  assert.equal(wasm.scan_markers(text), expected, 'Wasm fresh parity case '+i);
  assert.equal(cached.scan(), expected, 'Wasm preloaded parity case '+i);
  cached.free();
  const file = 'native/tmp/validation-'+i+'.md';
  writeFileSync(file, text);
  const run = spawnSync('./native/target/release/aquilum_native_scan', [
    file, String(expected), 'validation', '1', String(i),
  ],{cwd:process.cwd(),encoding:'utf8', timeout:20000});
  assert.equal(run.status, 0, 'Native Rust parity '+i+': '+run.stderr);
  const line = run.stdout.split('\n').find(l=>l.startsWith('AQUILUM_NATIVE_COMPARE '));
  assert.ok(line, 'Native receipt for '+i);
  const receipt = JSON.parse(line.slice('AQUILUM_NATIVE_COMPARE '.length));
  assert.equal(receipt.expected, expected, 'Native expected count '+i);
}
console.log('AQUILUM_NATIVE_PARITY cases='+tests.length+' modes=4 passed=true');
