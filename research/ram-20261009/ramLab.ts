/* Isolated real Tauri/WebView2 live tabs RAM scenario (not upstream UI). */
import { editorFor } from '../../components/Editor/openEditors';

const paths: string[] = String(import.meta.env.VITE_AQUILUM_RAM_NOTES || '').split('|');
const limit = Number(window.localStorage.getItem('aquilum_live_tab_limit'));
const URL = 'http://127.0.0.1:18713/event';
const pause = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms));

async function event(stage: string, details: Record<string, string | number | boolean | null> = {}) {
  const response = await fetch(URL, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ stage, elapsedMs: performance.now(), details: { liveTabLimit: limit, ...details } }),
  });
  if (!response.ok) throw new Error('Telemetry rejected stage ' + stage);
}
async function until<T>(fn: () => T | null, name: string, ms = 90000): Promise<T> {
  const deadline = performance.now() + ms;
  while (performance.now() < deadline) {
    const value = fn();
    if (value !== null) return value;
    await pause(100);
  }
  throw new Error('Timed out: ' + name);
}
function mounted() {
  return paths.filter((path) => editorFor(path) != null).length;
}
function editorHealthy(path: string) {
  const view = editorFor(path);
  return view && view.state.doc.length > 3_000_000 ? view : null;
}
async function snapshot(stage: string, expected: number) {
  await until(() => mounted() === expected ? true : null, 'live mounted editors='+ expected, 30000);
  await pause(2600);
  await event(stage, { mountedEditors: mounted(), expectedMounted: expected });
  await pause(1800);
}
async function scenario(): Promise<void> {
  if (![1, 3].includes(limit)) throw new Error('Unsupported liveTab limit ' + limit);
  if (paths.length !== 3 || paths.some((s) => !s)) throw new Error('Need three distinct 5MB fixtures');
  await event('renderer_initialized');
  await until(() => editorHealthy(paths[0]!), 'first 5MB editor', 120000);
  await snapshot('one_tab_stable', 1);

  for (let i = 1; i < paths.length; i++) {
    window.dispatchEvent(new CustomEvent('aquilum-ram-lab-new-tab', { detail: { path: paths[i] } }));
    await until(() => editorHealthy(paths[i]!), 'new editor ' + i);
    await snapshot(i === 1 ? 'two_tabs_stable' : 'three_tabs_stable', Math.min(limit, i + 1));
  }

  const clock = performance.now();
  window.dispatchEvent(new CustomEvent('aquilum-ram-lab-select-tab', { detail: { path: paths[0] } }));
  await until(() => editorHealthy(paths[0]!), 'switch to first large note');
  const switchMs = performance.now() - clock;
  await snapshot('switched_first_stable', Math.min(limit, 3));
  await event('switch_latency', { latencyMs: switchMs, mountedEditors: mounted() });
  await event('finished');
}

export async function runRamLab() {
  try { await scenario(); }
  catch (error) {
    try { await event('error', { message: String(error) }); } catch (nested) { console.error(nested); }
  }
}
