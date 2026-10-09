import { performance } from 'node:perf_hooks';
import { createRequire } from 'node:module';
import inspector from 'node:inspector';
import { fixtureOf } from './fixture.mjs';

const [variant, profile, mbString, roundString] = process.argv.slice(2);
const mb = Number(mbString);
const round = Number(roundString);
if (!['typescript', 'rescript', 'rustwasm'].includes(variant) || ![1, 5, 20].includes(mb)) {
  throw new Error('usage: measure-one variant profile size round');
}
if (typeof global.gc !== 'function') throw new Error('Run Node with --expose-gc');

const mib = (bytes) => Number((bytes / (1024 * 1024)).toFixed(3));
const mem = () => {
  const m = process.memoryUsage();
  return {
    rssMiB: mib(m.rss),
    heapUsedMiB: mib(m.heapUsed),
    externalMiB: mib(m.external),
    arrayBuffersMiB: mib(m.arrayBuffers),
  };
};
const gc = () => { global.gc(); global.gc(); };
const sortedAt = (xs, q) => [...xs].sort((a,b)=>a-b)[Math.ceil(q*xs.length)-1];
const fixture = fixtureOf(profile, mb);
gc();
const beforeImport = mem();
const init0 = performance.now();
let scan;
if (variant === 'typescript') {
  const mod = await import('./dist/ts.mjs');
  scan = mod.scanMarkers;
} else if (variant === 'rescript') {
  const mod = await import('./dist/rescript.mjs');
  scan = mod.scanMarkers;
} else {
  const require = createRequire(import.meta.url);
  const mod = require('./rust/pkg/aquilum_scan_bench.js');
  scan = mod.scan_markers;
}
const importMs = performance.now() - init0;
gc();
const afterImport = mem();
if (typeof scan !== 'function') throw new Error('missing scanner: ' + variant);
const first = scan(fixture.text);
if (first !== fixture.encodedResult) {
  throw new Error('scanner failed correctness fixture: got ' + first + ' expected ' + fixture.encodedResult);
}

for (let i=0; i<9; i++) {
  if (scan(fixture.text) !== fixture.encodedResult) throw new Error('warmup result changed');
}
gc();
const beforeCalls = mem();
const samples = [];
let checksum = 0;
const repetitions = 31;
let peakSampledRssMiB = beforeCalls.rssMiB;
for (let i = 0; i < repetitions; i++) {
  const started = performance.now();
  const value = scan(fixture.text);
  const ms = performance.now() - started;
  if (value !== fixture.encodedResult) throw new Error('invalid scan at iteration ' + i);
  checksum += value;
  samples.push(ms);
  if (i % 4 === 0) {
    peakSampledRssMiB = Math.max(peakSampledRssMiB, mem().rssMiB);
  }
}
const afterCallsBeforeGc = mem();
gc();
const afterCalls = mem();

function post(session, method, params={}) {
  return new Promise((resolve, reject) => {
    session.post(method, params, (error, result) => error ? reject(error) : resolve(result));
  });
}
async function sampledJsAllocations() {
  const session = new inspector.Session();
  session.connect();
  try {
    await post(session, 'HeapProfiler.enable');
    await post(session, 'HeapProfiler.startSampling', { samplingInterval: 8192 });
    for (let i = 0; i < 10; i++) {
      if (scan(fixture.text) !== fixture.encodedResult) throw new Error('sampled result changed');
    }
    const { profile: allocationProfile } = await post(session, 'HeapProfiler.stopSampling');
    let allocated = 0;
    function visit(node) {
      allocated += node.selfSize || 0;
      for (const child of node.children || []) visit(child);
    }
    visit(allocationProfile.head);
    return { sampledJsAllocBytesPerCall: Math.round(allocated / 10) };
  } finally {
    session.disconnect();
  }
}
const allocation = await sampledJsAllocations();

const result = {
  variant, profile, requestedMb:mb, round,
  utf8Bytes: fixture.utf8Bytes, utf16Units: fixture.utf16Units,
  expectedBooks: fixture.books, expectedQuotes: fixture.quotes,
  p50Ms: Number(sortedAt(samples,.5).toFixed(5)),
  p95Ms: Number(sortedAt(samples,.95).toFixed(5)),
  p99Ms: Number(sortedAt(samples,.99).toFixed(5)),
  meanMs: Number((samples.reduce((a,b)=>a+b,0)/samples.length).toFixed(5)),
  importMs: Number(importMs.toFixed(4)),
  ...allocation,
  beforeImport, afterImport, beforeCalls, afterCallsBeforeGc, afterCalls,
  rssPeakMiB: mib(process.resourceUsage().maxRSS * 1024),
  peakSampledRssMiB,
  checksum,
  nodeVersion: process.version,
};
console.log('AQUILUM_LANG_RESULT ' + JSON.stringify(result));
