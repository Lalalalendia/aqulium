import { mkdirSync, writeFileSync } from 'node:fs';
import { fixtureOf } from '../fixture.mjs';

const workloads = [
  ['mixed', 1], ['mixed', 5], ['dense', 5],
  ['ascii', 5], ['mixed', 20],
];
mkdirSync('native/tmp', { recursive: true });
const manifest = [];
for (const [profile, mb] of workloads) {
  const fixture = fixtureOf(profile, mb);
  const file = 'native/tmp/' + profile + '-' + mb + '.md';
  writeFileSync(file, fixture.text, 'utf8');
  manifest.push({
    profile, mb, file, utf8Bytes: fixture.utf8Bytes,
    utf16Units: fixture.utf16Units,
    expected: fixture.encodedResult,
    books: fixture.books, quotes: fixture.quotes,
  });
  console.log('AQUILUM_NATIVE_FIXTURE ' + JSON.stringify(manifest.at(-1)));
}
writeFileSync('native/tmp/manifest.json', JSON.stringify(manifest, null, 2) + '\n');
