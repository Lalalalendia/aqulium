#!/usr/bin/env python3
"""Test-only patch for liveTabs=1 versus liveTabs=3 memory experiments."""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
here = Path(__file__).resolve().parent
app = root / 'aquilum-app/src/App.tsx'
source = app.read_text(encoding='utf-8')
anchor = 'const liveTabIds = useLiveTabs(tabs, activeTabId, config?.editor.liveTabs ?? DEFAULT_LIVE_TABS);'
assert source.count(anchor) == 1, 'Expected fixed live tab limit entrypoint'
source = source.replace(anchor, """const liveTabLimit = import.meta.env.VITE_AQUILUM_RAM_LAB
    ? Number(window.localStorage.getItem('aquilum_live_tab_limit') ?? DEFAULT_LIVE_TABS)
    : (config?.editor.liveTabs ?? DEFAULT_LIVE_TABS);
  const liveTabIds = useLiveTabs(tabs, activeTabId, liveTabLimit);""")
anchor = '  }, [openNote, closeTab]);'
assert source.count(anchor) == 1, 'Expected original RAM harness listener'
source = source.replace(anchor, anchor + """
  useEffect(() => {
    if (!import.meta.env.VITE_AQUILUM_RAM_LAB) return;
    const onNew = (event: Event) => {
      const path = (event as CustomEvent<{path: string}>).detail.path;
      openNote(path, { disposition: 'new-tab' });
    };
    const onSelect = (event: Event) => {
      const path = (event as CustomEvent<{path: string}>).detail.path;
      openNote(path, { disposition: 'current' });
    };
    window.addEventListener('aquilum-ram-lab-new-tab', onNew);
    window.addEventListener('aquilum-ram-lab-select-tab', onSelect);
    return () => {
      window.removeEventListener('aquilum-ram-lab-new-tab', onNew);
      window.removeEventListener('aquilum-ram-lab-select-tab', onSelect);
    };
  }, [openNote]);
""")
app.write_text(source, encoding='utf-8')

main = root / 'aquilum-app/src/main.tsx'
source = main.read_text(encoding='utf-8')
anchor = 'ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render('
assert source.count(anchor) == 1, 'Missing renderer bootstrap'
boot = """if (import.meta.env.VITE_AQUILUM_RAM_LAB) {
  const request = new XMLHttpRequest();
  request.open('GET', 'http://127.0.0.1:18713/mode', false);
  request.send();
  if (request.status !== 200) throw new Error('Missing live tab RAM benchmark mode');
  window.localStorage.setItem('aquilum_live_tab_limit', request.responseText);
}
"""
main.write_text(source.replace(anchor, boot + '\n' + anchor), encoding='utf-8')
lab = root / 'aquilum-app/src/modules/perf/ramLab.ts'
assert lab.exists()
lab.write_bytes((here / 'liveTabsLab.ts').read_bytes())
print('Test-only runtime switch installed before React render')
