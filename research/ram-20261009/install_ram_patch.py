#!/usr/bin/env python3
"""Apply test-only modifications to a pinned Aquilum checkout.
Do not commit these changes to Freaction/Aquilum.
"""
from pathlib import Path
import sys

root = Path(sys.argv[1]).resolve()
mode = sys.argv[2].strip()
here = Path(__file__).resolve().parent

if mode == "native":
    mod = root / "aquilum-app/core/src/documents/mod.rs"
    contents = mod.read_text(encoding="utf-8")
    anchor = "pub use hub::{DocumentEvent, DocumentHub, SaveFailed};"
    assert contents.count(anchor) == 1, "Unexpected documents mod.rs"
    contents = contents.replace(anchor, anchor + "\n#[cfg(all(test, windows))]\nmod native_memory_lab;\n")
    mod.write_text(contents, encoding="utf-8")
    (mod.parent / "native_memory_lab.rs").write_bytes((here / "native_memory_lab.rs").read_bytes())
elif mode == "desktop":
    app = root / "aquilum-app/src/App.tsx"
    contents = app.read_text(encoding="utf-8")
    anchor = 'import { lazy, Suspense, useCallback, useEffect, useState } from "react";'
    assert contents.count(anchor) == 1, "Unexpected App imports"
    contents = contents.replace(anchor, anchor.replace("useState", "useState, useRef"))
    anchor = "  useWindowDocumentSync();"
    assert contents.count(anchor) == 1, "Unexpected App layout"
    effects = """
  // Ephemeral runner-only test mode. It is absent from upstream builds.
  const labBooted = useRef(false);
  const labOpened = useRef(false);
  useEffect(() => {
    if (!import.meta.env.VITE_AQUILUM_RAM_LAB || workspaceRestoring || labBooted.current) return;
    labBooted.current = true;
    void openWorkspace(import.meta.env.VITE_AQUILUM_RAM_VAULT);
  }, [workspaceRestoring, openWorkspace]);
  useEffect(() => {
    if (!import.meta.env.VITE_AQUILUM_RAM_LAB || !workspaceReady || !sessionReady || labOpened.current) return;
    labOpened.current = true;
    openNote(import.meta.env.VITE_AQUILUM_RAM_NOTE);
  }, [workspaceReady, sessionReady, openNote]);
  useEffect(() => {
    if (!import.meta.env.VITE_AQUILUM_RAM_LAB) return;
    const onOpen = () => openNote(import.meta.env.VITE_AQUILUM_RAM_NOTE);
    const onClose = () => closeTab(import.meta.env.VITE_AQUILUM_RAM_NOTE);
    window.addEventListener('aquilum-ram-lab-open', onOpen);
    window.addEventListener('aquilum-ram-lab-close', onClose);
    return () => {
      window.removeEventListener('aquilum-ram-lab-open', onOpen);
      window.removeEventListener('aquilum-ram-lab-close', onClose);
    };
  }, [openNote, closeTab]);

"""
    contents = contents.replace(anchor, effects + anchor)
    app.write_text(contents, encoding="utf-8")
    main = root / "aquilum-app/src/main.tsx"
    contents = main.read_text(encoding="utf-8")
    assert "beginBootTrace();" in contents and "runRamLab" not in contents
    contents += """
// Test-only in-renderer probe. Enabled only in the isolated Windows CI build.
if (import.meta.env.VITE_AQUILUM_RAM_LAB) {
  void import('./modules/perf/ramLab').then((module) => module.runRamLab());
}
"""
    main.write_text(contents, encoding="utf-8")
    (main.parent / "modules/perf/ramLab.ts").write_bytes((here / "ramLab.ts").read_bytes())
else:
    raise SystemExit("mode must be 'native' or 'desktop'")

print("Applied ephemeral Aquilum RAM lab patch, mode=" + mode)
