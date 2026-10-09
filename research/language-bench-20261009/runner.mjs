import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';

const sizes = {
  typescript: ['dist/ts.mjs'],
  rescript: ['dist/rescript.mjs'],
  rustwasm: ['rust/pkg/aquilum_scan_bench.js', 'rust/pkg/aquilum_scan_bench_bg.wasm'],
};
const sizeResults = Object.fromEntries(Object.entries(sizes).map(([variant, files]) => {
  let raw = 0, gzip = 0;
  for (const path of files) {
    const b = readFileSync(path);
    raw += b.byteLength;
    gzip += gzipSync(b, { level:9 }).byteLength;
  }
  return [variant, {rawBytes:raw,gzipBytes:gzip,files}];
}));

const profiles = [
  ['mixed',1], ['mixed',5], ['dense',5], ['mixed',20],
];
const variants = ['typescript','rescript','rustwasm'];
const results = [];
for (const [profile,mb] of profiles) {
  for (let round = 0; round < 2; round++) {
    const order = round ? [...variants].reverse() : variants;
    for (const variant of order) {
      const call = spawnSync(process.execPath, [
        '--expose-gc','measure-one.mjs',variant,profile,String(mb),String(round),
      ], {cwd:process.cwd(),encoding:'utf8',timeout:300000,maxBuffer:1024*1024*5});
      if (call.error || call.status !== 0) {
        throw new Error(variant + '/' + profile + '/' + mb + ' child failed: '
          + (call.error?.message||'') + '\nstdout:\n' + call.stdout + '\nstderr:\n' + call.stderr);
      }
      const line = call.stdout.split('\n').find(x=>x.startsWith('AQUILUM_LANG_RESULT '));
      if (!line) throw new Error('no verified child result: '+ call.stdout);
      const result=JSON.parse(line.slice('AQUILUM_LANG_RESULT '.length));
      results.push(result);
      console.log(line);
    }
  }
}
const avg = values=>values.reduce((a,b)=>a+b,0)/values.length;
const asNum = (v,n=3)=>Number(v.toFixed(n));
const summary=[];
for (const [profile,mb] of profiles) {
  for (const variant of variants) {
    const subset=results.filter(r=>r.profile===profile&&r.requestedMb===mb&&r.variant===variant);
    if(subset.length!==2)throw Error('missing repeat: '+variant+profile+mb);
    summary.push({
      variant,profile,mb,utf8Bytes:subset[0].utf8Bytes,
      p50Ms:asNum(avg(subset.map(x=>x.p50Ms)),5),
      p95Ms:asNum(avg(subset.map(x=>x.p95Ms)),5),
      importMs:asNum(avg(subset.map(x=>x.importMs)),3),
      rssPeakMiB:asNum(avg(subset.map(x=>x.rssPeakMiB)),3),
      heapUsedAfterGcMiB:asNum(avg(subset.map(x=>x.afterCalls.heapUsedMiB)),3),
      wasmExternalAfterGcMiB:asNum(avg(subset.map(x=>x.afterCalls.externalMiB)),3),
      jsSampledAllocBytesPerCall:asNum(avg(subset.map(x=>x.sampledJsAllocBytesPerCall)),1),
      ...sizeResults[variant],
    });
  }
}
mkdirSync('output',{recursive:true});
const metadata={generated:new Date().toISOString(),
  benchmark:'line-oriented Markdown book/reader quote-header prefilter, NOT full parsing or UI rendering',
  host:'GitHub Actions ubuntu-latest, Node.js 22, --expose-gc, each language separate process',
  methodology:'31 timed iterations after 9 warmups, 2 independent child runs per workload; averages of per-run p50 and p95',
  wasmInterop:'Rust/Wasm called with JS string on every iteration, encoding and copying into Wasm included',
  memoryCaveat:'RSS may contain shared/code pages and retained heap; sampled JS heap allocation excludes Wasm linear memory',
  nodeVersion:process.version,
  sizes:sizeResults,results,summary};
writeFileSync('output/results.json',JSON.stringify(metadata,null,2)+'\n');
const headers=['variant','profile','mb','utf8Bytes','p50Ms','p95Ms','importMs','rssPeakMiB',
  'heapUsedAfterGcMiB','wasmExternalAfterGcMiB','jsSampledAllocBytesPerCall','rawBytes','gzipBytes'];
writeFileSync('output/results.csv',headers.join(',')+'\n'+summary.map(r=>headers.map(x=>r[x]).join(',')).join('\n')+'\n');

const md=[
 '# Aquilum: TypeScript vs ReScript vs Rust/Wasm benchmark',
 '',
 'Controlled run of the same lexical quote-header prefilter on real-size Markdown strings. Two separate native Node processes per language / workload; 31 timed calls per process after JIT warmup. Compiler and package versions in build logs.',
 '',
 '| Workload | Language | p50 (ms) | p95 (ms) | RSS peak (MiB) | JS heap after GC (MiB) | Sampled JS allocations / call (bytes) | Raw deliverable | Gzip |',
 '|---|---|---:|---:|---:|---:|---:|---:|---:|',
 ...summary.map(r=>'| '+r.profile+' '+r.mb+' MB | '+r.variant+' | '+r.p50Ms+' | '+r.p95Ms+
     ' | '+r.rssPeakMiB+' | '+r.heapUsedAfterGcMiB+' | '+r.jsSampledAllocBytesPerCall+
     ' | '+r.rawBytes+' | '+r.gzipBytes+' |'),
 '',
 '## Important scope and limits',
 '',
 '- All three were checked against an independent regex oracle on mixed-case, Cyrillic, Japanese, emoji, CRLF and typical marker syntax. This is a lexical prefilter; it is not the full CodeMirror quoteScanPresence, isBookCalloutHeader or reader quote parser.',
 '- Rust runs as WebAssembly inside Node, not as a native executable. JS string to UTF-8 Wasm marshaling/copying is counted in every operation.',
 '- The measured p50 is a hot scanner call. It is not actual keyboard-to-pixel UI latency nor whole-app idle memory.',
 '- RSS peak is entire Node process high-water; it includes the common input string and module dependencies. JS allocations come from V8 sampling after timing and exclude native/Wasm linear memory.',
 '- Build size is minified bundled TS or ReScript JS versus WASM binary + required JS glue. Gzip totals are the sum of each shipping file.',
 '- Results are two runner-local runs, not a statistical guarantee across CPUs or browsers. Investigate WebView2 separately before any language migration.',
];
writeFileSync('output/results.md',md.join('\n')+'\n');
console.log('AQUILUM_LANG_SUMMARY '+JSON.stringify(summary));
console.log('AQUILUM_LANG_REPORT output/results.md');
