#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, '..');

const packageJson = JSON.parse(await readFile(join(projectRoot, 'package.json'), 'utf8'));
const version = packageJson.version;
if (!version) throw new Error('package.json has no version field');

console.log(`\x1b[36mWindows release v${version} — локальная сборка\x1b[0m`);
console.log('\x1b[90mGitHub Actions для релиза отключён (лимит минут на private-репо).\x1b[0m\n');

execFileSync(process.execPath, ['scripts/build-release.mjs'], {
  cwd: projectRoot,
  stdio: 'inherit',
});
