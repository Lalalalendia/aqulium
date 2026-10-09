import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';
import { createRequire } from 'node:module';
import inspector from 'node:inspector';

const [variant, file, expectedString, profile, mbString, roundString] = process.argv.slice(2);
if (!['typescript', 'wasm_fresh', 'wasm_cached'].includes(variant)) {
  throw Error('Expected typescript/wasm_fresh/wasm_cached');
}
if (typeof global.gc !== 'function') throw Error('Need Node --expose-gc');
const expected = Number(expectedString);
const mb = Number(mbString);
const round = Number(roundString);
const rounds = mb === 1 ? 81 : mb === 5 ? 61 : 41;
const mib = (bytes) => Number((bytes / (1024 * 1024)).toFixed(3));
function memory() {
  const m = process.memoryUsage();
  return {
    rssMiB: mib(m.rss),
    heapUsedMiB: mib(m.heapUsed),
    externalMiB: mib(m.external),
    arrayBuffersMiB: mib(m.arrayBuffers),
  };
}
function peakRss() { return mib(process.resourceUsage().maxRSS * 1024); }
function collect() { global.gc(); global.gc(); }
function quantile(numbers, q) {
  const copy = [...numbers].sort((a, b) => a - b);
  return copy[Math.ceil(numbers.length * q) - 1];
}

collect();
const beforeLoad = memory();
const text = readFileSync(file, 'utf8');
const utf8Bytes = Buffer.byteLength(text, 'utf8');
const afterLoad = memory();

const startedImport = performance.now();
let scan;
let preloadMs = 0;
let linearMemoryBytes = null;
let cleanup;
if (variant === 'typescript') {
  const mod = await import('../dist/ts.mjs');
  scan = () => mod.scanMarkers(text);
} else {
  const require = createRequire(import.meta.url);
  const wasm = require('../rust/pkg/aquilum_scan_bench.js');
  if (variant === 'wasm_cached') {
    const startedPreload = performance.now();
    const loaded = new wasm.PreloadedScanner(text);
    preloadMs = performance.now() - startedPreload;
    if (loaded.byte_len() !== utf8Bytes) throw Error('Wasm preloaded bytes differ');
    scan = () => loaded.scan();
    cleanup = () => loaded.free();
  } else {
    scan = () => wasm.scan_markers(text);
  }
  // Depending on wasm-bindgen version, the memory object may not be exposed
  // through the high-level JS glue. External memory counters remain recorded.
  linearMemoryBytes = wasm.__wasm?.memory?.buffer?.byteLength ?? null;
}
const importAndPreloadMs = performance.now() - startedImport;
collect();
const afterImport = memory();

if (scan() !== expected) throw Error('Incorrect result before warmup for ' + variant);
for (let i = 0; i < 10; i++) {
  if (scan() !== expected) throw Error('Incorrect warmup result');
}
collect();
const afterWarm = memory();
const elapsed = [];
let checksum = 0;
for (let i = 0; i < rounds; i++) {
  const begin = performance.now();
  const result = scan();
  const duration = performance.now() - begin;
  if (result !== expected) throw Error('Mismatching scanner result');
  checksum += result;
  elapsed.push(duration);
}
const afterTimedBeforeGc = memory();
collect();
const afterTimedGc = memory();
const highWaterBeforeProfiling = peakRss();

// Inspector sampling is OUTSIDE of the measured hot path. Its numbers cover
// sampled V8 heap allocations, not Rust/Wasm linear memory allocations.
function post(session, method, parameters={}) {
  return new Promise((resolve, reject) => {
    session.post(method, parameters, (error, result) => error ? reject(error) : resolve(result));
  });
}
async function sampledV8Allocations() {
  const session = new inspector.Session();
  session.connect();
  try {
    await post(session, 'HeapProfiler.enable');
    await post(session, 'HeapProfiler.startSampling', { samplingInterval: 4096 });
    const samples = 10;
    for (let i = 0; i < samples; i++) {
      if (scan() !== expected) throw Error('Invalid result during allocation sampling');
    }
    const { profile: allocationProfile } = await post(session, 'HeapProfiler.stopSampling');
    let bytes = 0;
    const walk = (node) => {
      bytes += node.selfSize || 0;
      for (const next of node.children || []) walk(next);
    };
    walk(allocationProfile.head);
    return Math.round(bytes / samples);
  } finally {
    session.disconnect();
  }
}
const sampledJsAllocBytesPerCall = await sampledV8Allocations();

const data = {
  variant, profile, requestedMb: mb, round, utf8Bytes, utf16Units:text.length,
  expected, repetitions:rounds,
  p50Ms:Number(quantile(elapsed, 0.5).toFixed(6)),
  p95Ms:Number(quantile(elapsed, 0.95).toFixed(6)),
  p99Ms:Number(quantile(elapsed, 0.99).toFixed(6)),
  meanMs:Number((elapsed.reduce((x,y)=>x+y, 0)/elapsed.length).toFixed(6)),
  importAndPreloadMs:Number(importAndPreloadMs.toFixed(4)),
  oneTimePreloadMs:Number(preloadMs.toFixed(4)),
  beforeLoad, afterLoad, afterImport, afterWarm, afterTimedBeforeGc, afterTimedGc,
  rssPeakMiB:highWaterBeforeProfiling,
  wasmLinearMemoryBytes:linearMemoryBytes,
  sampledJsAllocBytesPerCall, checksum,
  nodeVersion:process.version,
};
console.log('AQUILUM_NATIVE_COMPARE ' + JSON.stringify(data));
if (cleanup) cleanup();
