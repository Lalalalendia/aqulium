import { build } from 'esbuild';
import { mkdirSync, existsSync } from 'node:fs';

mkdirSync('dist', { recursive: true });
const sources = [
  ['src/scan.ts', 'dist/ts.mjs'],
  ['src/QuoteScan.res.js', 'dist/rescript.mjs'],
];
for (const [entry, outfile] of sources) {
  if (!existsSync(entry)) throw new Error('Missing compiled entry ' + entry);
  await build({
    entryPoints: [entry],
    bundle: true,
    format: 'esm',
    platform: 'node',
    target: 'node22',
    minify: true,
    legalComments: 'none',
    outfile,
    sourcemap: false,
    logLevel: 'info',
  });
}
