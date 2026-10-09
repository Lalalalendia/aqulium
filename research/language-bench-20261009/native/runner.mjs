import { spawnSync } from 'node:child_process';
import { readFileSync, writeFileSync, mkdirSync, statSync } from 'node:fs';
import { gzipSync } from 'node:zlib';
import { cpus } from 'node:os';

const manifest = JSON.parse(readFileSync('native/tmp/manifest.json', 'utf8'));
const variants = ['typescript', 'rust_native', 'wasm_fresh', 'wasm_cached'];
const rounds = 3;
const results = [];
const base = 'AQUILUM_NATIVE_COMPARE ';
const exe = 'native/target/release/aquilum_native_scan';

for (const workload of manifest) {
  const inputText = readFileSync(workload.file, 'utf8');
  const actualBook = (inputText.match(/^[ \t]*>[ \t]*\[!book\]/gim) || []).length;
  const actualQuote = (inputText.match(/^[ \t]*>[ \t]*\[!quote\]/gim) || []).length;
  if (actualBook * 65536 + actualQuote !== workload.expected) {
    throw new Error('Independent regex oracle disagrees on ' + workload.file);
  }
  if (Buffer.byteLength(inputText) !== workload.utf8Bytes) throw Error('file bytes mismatch');

  for (let round = 0; round < rounds; round++) {
    const order = round === 0 ? [...variants]
      : round === 1 ? [...variants].reverse()
      : ['wasm_cached', 'typescript', 'wasm_fresh', 'rust_native'];
    for (const variant of order) {
      let command, args;
      if (variant === 'rust_native') {
        command = exe;
        args = [workload.file, String(workload.expected),
          workload.profile, String(workload.mb), String(round)];
      } else {
        command = process.execPath;
        args = [
          '--expose-gc', 'native/measure-one.mjs',
          variant, workload.file, String(workload.expected),
          workload.profile, String(workload.mb), String(round),
        ];
      }
      const processResult = spawnSync(command, args, {
        cwd: process.cwd(), encoding: 'utf8',
        timeout: 300000, maxBuffer: 1024 * 1024 * 6,
      });
      if (processResult.error || processResult.status !== 0) {
        throw new Error('Bench failed: '+ variant+'/'+workload.profile+' '+workload.mb+' MB round '+round
          + '\n' + String(processResult.error ?? '')
          + '\nSTDOUT: '+ processResult.stdout
          + '\nSTDERR: '+ processResult.stderr);
      }
      const line = processResult.stdout.split('\n').find(x => x.startsWith(base));
      if (!line) throw Error('No scanner result: '+variant+'\n'+processResult.stdout);
      const datum = JSON.parse(line.slice(base.length));
      if (datum.expected !== workload.expected || datum.utf8Bytes !== workload.utf8Bytes ||
          datum.profile !== workload.profile || datum.requestedMb !== workload.mb ||
          datum.variant !== variant || datum.round !== round) {
        throw Error('Measurement metadata mismatch '+line);
      }
      if (datum.checksum !== datum.expected * datum.repetitions) {
        throw Error('Checksum mismatch '+line);
      }
      results.push(datum);
      console.log(line);
    }
  }
}
function median(values) {
  if (!values.length) throw Error('Empty data');
  const sorted = [...values].sort((a,b) => a-b);
  return sorted[Math.floor(sorted.length/2)];
}
function num(n, precision=4) { return Number(n.toFixed(precision)); }
const summary=[];
for (const workload of manifest) {
  for (const variant of variants) {
    const cases = results.filter(r=>r.profile===workload.profile &&
      r.requestedMb===workload.mb && r.variant===variant);
    if (cases.length !== rounds) throw Error('Missing independent runs');
    const get = key=>median(cases.map(r=>r[key]));
    const native = variant==='rust_native';
    const medianRssBaseline = median(cases.map(r =>
      native ? r.rssBeforeLoadMiB : r.beforeLoad.rssMiB));
    const medianRssWarm = median(cases.map(r =>
      native ? r.rssAfterWarmMiB : r.afterWarm.rssMiB));
    summary.push({
      profile:workload.profile,mb:workload.mb,utf8Bytes:workload.utf8Bytes,
      variant,runs:cases.length,repetitionsPerRun:cases[0].repetitions,
      p50Ms:num(get('p50Ms'),6), p95Ms:num(get('p95Ms'),6),
      medianRssPeakMiB:num(get('rssPeakMiB'),3),
      medianRssDeltaLoadedMiB:num(medianRssWarm-medianRssBaseline,3),
      medianPreloadMs:native?null:num(get('oneTimePreloadMs'),3),
      nativeAllocCallsPerScan:native?num(get('nativeAllocCallsPerScan'),4):null,
      nativeAllocatedBytesPerScan:native?num(get('nativeAllocatedBytesPerScan'),3):null,
      v8SampledJsAllocBytesPerScan:native?null:num(get('sampledJsAllocBytesPerCall'),1),
      medianHeapAfterGcMiB:native?null:num(median(cases.map(r=>r.afterTimedGc.heapUsedMiB)),3),
      medianExternalAfterGcMiB:native?null:num(median(cases.map(r=>r.afterTimedGc.externalMiB)),3),
    });
  }
}
for (const workload of manifest) {
  const baseline=summary.find(x=>x.profile===workload.profile&&x.mb===workload.mb&&x.variant==='typescript');
  for (const rec of summary.filter(x=>x.profile===workload.profile&&x.mb===workload.mb)) {
    rec.speedupVsTypeScript=num(baseline.p50Ms/rec.p50Ms,3);
  }
}
const fileSizes={
  typescript:['dist/ts.mjs'],
  rust_native:[exe],
  wasm_fresh:['rust/pkg/aquilum_scan_bench.js','rust/pkg/aquilum_scan_bench_bg.wasm'],
  wasm_cached:['rust/pkg/aquilum_scan_bench.js','rust/pkg/aquilum_scan_bench_bg.wasm'],
};
const sizes=Object.fromEntries(Object.entries(fileSizes).map(([name,paths])=>{
  const bytes=paths.reduce((sum,path)=>sum+statSync(path).size,0);
  const gzipBytes=paths.reduce((sum,path)=>sum+gzipSync(readFileSync(path),{level:9}).length,0);
  return [name,{rawBytes:bytes,gzipBytes,files:paths}];
}));
for(const item of summary) Object.assign(item,{rawBytes:sizes[item.variant].rawBytes,gzipBytes:sizes[item.variant].gzipBytes});
mkdirSync('native/output',{recursive:true});
const report={
  generatedUtc:new Date().toISOString(),
  context:'Native Rust vs V8 TypeScript vs Wasm fresh vs Wasm preloaded; identical Markdown lexical header scan; shared Rust kernel compiled both ways.',
  methodology:'3 separate process runs per variant/corpus, 10 warmups, 81/61/41 timed calls for 1/5/20 MB; process order reversed or rotated per run.',
  important:'JS strings are UTF-16, native Rust and Wasm scan UTF-8 of the same text. Wasm cached copies once before timing; fresh Wasm converts and copies per scan. Native Rust reads the same UTF-8 file once before timing.',
  memoryCaveat:'RSS high-water is per-process memory, not application WebView2 usage or unique physical RAM. Native allocator counts only native timed scan; V8 sampled heap allocation excludes native/Wasm linear memory.',
  host:{nodeVersion:process.version,cpus:cpus().slice(0,1).map(c=>c.model),arch:process.arch,platform:process.platform},
  manifest,codeSizes:sizes,summary,rawResults:results,
};
writeFileSync('native/output/results.json',JSON.stringify(report,null,2)+'\n');
const cols=['profile','mb','variant','runs','p50Ms','p95Ms','speedupVsTypeScript','medianRssPeakMiB','medianRssDeltaLoadedMiB','medianPreloadMs','nativeAllocCallsPerScan','nativeAllocatedBytesPerScan','v8SampledJsAllocBytesPerScan','rawBytes','gzipBytes'];
writeFileSync('native/output/results.csv',cols.join(',')+'\n'+summary.map(o=>cols.map(x=>o[x]??'').join(',')).join('\n')+'\n');
const md=[
'# Aquilum: native Rust vs TypeScript V8 vs Rust/Wasm (fresh and preloaded)',
'',
'Same [!book]/[!quote] Markdown lexical prefilter on exact same text. Three fresh processes per variant/workload, 10 warmups, multiple calls in each process. Hot-call median p50 for each set is aggregated as the median of three per-process medians.',
'',
'| Text | Variant | p50 (ms) | p95 (ms) | vs TypeScript | Peak RSS (MiB) | RSS growth loaded/warmed (MiB) |',
'|---|---|---:|---:|---:|---:|---:|',
...summary.map(r=>'| '+r.profile+' '+r.mb+' MB | '+r.variant+' | '+r.p50Ms+' | '+r.p95Ms
  +' | '+r.speedupVsTypeScript+'x | '+r.medianRssPeakMiB+' | '+r.medianRssDeltaLoadedMiB+' |'),
'',
'## Code/package size',
'',
'| Executable/deliverable | Raw bytes | Gzip bytes | Files |',
'|---|---:|---:|---|',
...Object.entries(sizes).map(([key,val])=>'| '+key+' | '+val.rawBytes+' | '+val.gzipBytes+' | '+val.files.join(', ')+' |'),
'',
'## Correct interpretation',
'',
'- Native Rust and Wasm use the exact SAME Rust kernel source. TypeScript uses its previous V8 code. Both recognize identical headers across the same Markdown Unicode files.',
'- Native Rust reads UTF-8 input before timing. TypeScript uses an already-decoded JS UTF-16 string. Wasm-fresh re-encodes/copies that JS string each call; Wasm-cached copies once outside timing and then scans its own retained buffer.',
'- Preload times and JS external memory are in results.json. Native allocation counters are for timed scanning only; V8 sampled allocation counters refer only to sampled JS heap, not Wasm or Rust allocator.',
'- Whole-process RSS is not the same as memory consumed by one application module. A native CLI does not run WebView2 or Node; compare speed and incremental RAM separately from baseline process overhead.',
'- A narrow lexical scan does not prove anything about full CodeMirror rendering, asynchronous IPC, search integration or replacing the entire Aquilum TypeScript frontend.',
'- RSS and GC can fluctuate between runs; medians from three same-host subprocesses are a focused microbenchmark, not a universal language ranking.',
];
writeFileSync('native/output/results.md',md.join('\n')+'\n');
console.log('AQUILUM_NATIVE_SUMMARY ' + JSON.stringify(summary));
console.log('AQUILUM_NATIVE_FINISHED cases='+summary.length+' raw_runs='+results.length);
