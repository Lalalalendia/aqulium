/* Ephemeral actual CodeMirror/Tauri app RAM test. Not an upstream feature. */
import { editorFor } from '../../components/Editor/openEditors';

type Details = Record<string, string | number | boolean | null>;
const NOTE_PATH: string = import.meta.env.VITE_AQUILUM_RAM_NOTE;
const URL = 'http://127.0.0.1:18713/event';
const AB_KEY = 'aquilum_editor_ab_mode';
const AB_MODES = ['baseline', 'no_fullscans', 'no_renumber', 'both'] as const;
const AB_MODE = window.localStorage.getItem(AB_KEY) ?? 'baseline';


const pause = (ms: number): Promise<void> =>
  new Promise((resolve) => setTimeout(resolve, ms));
const frame = (): Promise<void> =>
  new Promise((resolve) => requestAnimationFrame(() => resolve()));

async function event(stage: string, details: Details = {}): Promise<void> {
  try {
    await fetch(URL, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ stage, elapsedMs: performance.now(), details: { mode: AB_MODE, ...details } }),
    });
  } catch (error) {
    // The Windows runner treats missing events as a failed/inconclusive experiment.
    console.error('[aquilum-ram-lab] Could not report stage', stage, error);
  }
}

function stats(values: number[]): Details {
  const sorted = [...values].sort((a, b) => a - b);
  const at = (fraction: number): number =>
    sorted[Math.min(sorted.length - 1, Math.floor(fraction * (sorted.length - 1)))];
  return {
    count: sorted.length,
    meanMs: values.reduce((a, b) => a + b, 0) / values.length,
    p50Ms: at(0.5), p95Ms: at(0.95), p99Ms: at(0.99),
    maxMs: sorted[sorted.length - 1],
  };
}

async function until<T>(work: () => T | null, timeoutMs: number, name: string): Promise<T> {
  const deadline = performance.now() + timeoutMs;
  while (performance.now() < deadline) {
    const result = work();
    if (result !== null) return result;
    await pause(100);
  }
  throw new Error('Timed out waiting for ' + name + ' after ' + timeoutMs + 'ms');
}

async function scenario(): Promise<void> {
  await event('renderer_initialized');
  const view = await until(() => editorFor(NOTE_PATH), 120_000, 'CodeMirror editor');
  const chars = view.state.doc.length;
  if (chars < 3_000_000) throw new Error('Fixture unexpectedly small: ' + chars);
  await event('editor_mounted_5mb', { characters: chars });
  await pause(1700);

  const dispatch = [] as number[];
  const paint = [] as number[];
  for (let i = 0; i < 120; i += 1) {
    const before = performance.now();
    const pos = view.state.doc.length;
    view.dispatch({ changes: { from: pos, insert: 'x' }, selection: { anchor: pos + 1 } });
    dispatch.push(performance.now() - before);
    await frame();
    paint.push(performance.now() - before);
    await pause(7);
  }
  const d = stats(dispatch), p = stats(paint);
  await event('after_120_editor_transactions', {
    dispatchP50Ms: Number(d.p50Ms), dispatchP95Ms: Number(d.p95Ms), dispatchP99Ms: Number(d.p99Ms),
    frameP50Ms: Number(p.p50Ms), frameP95Ms: Number(p.p95Ms), frameP99Ms: Number(p.p99Ms),
  });
  await pause(1800);

  const scrollFrames = [] as number[];
  const scrollNode = view.scrollDOM;
  scrollNode.scrollTop = 0;
  for (let i = 0; i < 140; i += 1) {
    const before = performance.now();
    scrollNode.scrollTop = (i % 2 === 0)
      ? Math.min(scrollNode.scrollHeight, scrollNode.scrollTop + 7_000)
      : Math.max(0, scrollNode.scrollTop - 2_000);
    await frame();
    scrollFrames.push(performance.now() - before);
  }
  const sf = stats(scrollFrames);
  await event('after_140_scroll_frames', {
    frameP50Ms: Number(sf.p50Ms), frameP95Ms: Number(sf.p95Ms), frameP99Ms: Number(sf.p99Ms),
    slowOver50Ms: scrollFrames.filter((ms) => ms > 50).length,
  });
  await pause(1700);

  window.dispatchEvent(new Event('aquilum-ram-lab-close'));
  await until(() => editorFor(NOTE_PATH) === null ? true : null, 20_000, 'first close');
  await pause(1000);
  await event('after_first_close');
  await pause(1400);

  for (let i = 0; i < 10; i += 1) {
    window.dispatchEvent(new Event('aquilum-ram-lab-open'));
    const loaded = await until(() => editorFor(NOTE_PATH), 25_000, 'reopen ' + i);
    if (loaded.state.doc.length < chars) throw new Error('Lost document contents after reopening');
    window.dispatchEvent(new Event('aquilum-ram-lab-close'));
    await until(() => editorFor(NOTE_PATH) === null ? true : null, 25_000, 'close ' + i);
  }
  await pause(1700);
  await event('after_ten_reopens');
  await pause(1300);
  const currentIndex = AB_MODES.findIndex((mode) => mode === AB_MODE);
  window.localStorage.setItem(AB_KEY, AB_MODES[(currentIndex + 1) % AB_MODES.length]!);
  await event('finished');
}

export async function runRamLab(): Promise<void> {
  try { await scenario(); }
  catch (error) { await event('error', { message: String(error) }); }
}
