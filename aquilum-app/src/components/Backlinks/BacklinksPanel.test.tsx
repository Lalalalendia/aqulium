// @vitest-environment happy-dom
import { act } from 'preact/test-utils';
import { mountDom, type MountedDom } from '../../testing/mountDom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BacklinksPanel } from './BacklinksPanel';

const linkMocks = vi.hoisted(() => ({
  backlink: {
    path: 'C:\\notes\\Source.md',
    title: 'Source',
    offset: 3,
  },
  outgoing: {
    target: 'Target',
    title: 'Target',
    path: 'C:\\notes\\Target.md',
  },
  analysis: {
    path: 'C:\\notes\\Related.md',
    title: 'Related',
    rawScore: 0.255,
    reasons: ['related'],
  },
}));

vi.mock('../../i18n', () => ({ t: (key: string) => key }));
vi.mock('../../modules/settings', () => ({
  useSettingsStore: () => ({
    config: {
      analysis: {
        enableBm25f: true,
        enableAdamicAdar: true,
        enableWikixiv: true,
        bm25fParams: {
          k1: 1.2,
          k3: 8,
          bTitle: 0.3,
          bBody: 0.75,
          titleWeight: 2.5,
        },
      },
    },
  }),
}));
vi.mock('../../modules/links', () => ({
  useDocumentLinks: (mode: 'backlinks' | 'outgoing') => ({
    result: mode === 'backlinks'
      ? { mode, items: [linkMocks.backlink] }
      : { mode, items: [linkMocks.outgoing] },
    loading: false,
  }),
}));
vi.mock('../../modules/analysis', () => ({
  enabledSidebarMethods: () => ['bm25f', 'adamicAdar', 'wixiv'],
  isGraphAnalysisMethod: (method: string) => method !== 'wixiv',
  useDocumentAnalysis: () => ({
    items: [linkMocks.analysis],
    failed: false,
    loading: false,
  }),
}));
vi.mock('../../modules/wikixiv', () => ({
  useWikixivSources: () => ({
    hits: [],
    offline: false,
    insufficientText: false,
    failed: false,
    loading: false,
  }),
  setWikiHover: () => {},
  clearWikiHover: () => {},
}));
vi.mock('../../modules/openExternalUrl', () => ({
  openExternalUrl: vi.fn(),
}));

describe('BacklinksPanel', () => {
  let renderer: MountedDom | null = null;

  beforeEach(() => {
    try {
      localStorage.clear();
    } catch {}
  });

  afterEach(() => {
    if (renderer) act(() => renderer?.unmount());
    renderer = null;
  });

  it('switches the pressed mode and renders document-name rows', () => {
    const onOpenBacklink = vi.fn();
    const onOpenOutgoing = vi.fn();
    const onOpenAnalysis = vi.fn();
    act(() => {
      renderer = mountDom(
        <BacklinksPanel
          workspacePath="C:\\notes"
          documentPath="C:\\notes\\Current.md"
          activeTabId={null}
          indexReady
          indexRevision={1}
          isOpen
          onOpenBacklink={onOpenBacklink}
          onOpenOutgoing={onOpenOutgoing}
          onOpenAnalysis={onOpenAnalysis}
        />,
      );
    });

    const panel = renderer!.container;
    const modeButtons = [...panel.querySelectorAll<HTMLButtonElement>('.q-backlinks__mode-button')];
    const pressed = () => modeButtons.map((button) => button.getAttribute('aria-pressed'));
    expect(pressed()).toEqual(['true', 'false', 'false', 'false']);
    expect(panel.querySelector('h2')?.textContent).toBe('backlinks.mentionsTitle');
    expect(panel.querySelector('.q-sidebar-document-item__title')?.textContent).toBe('Source');

    act(() => modeButtons[1].click());

    expect(pressed()).toEqual(['false', 'true', 'false', 'false']);
    expect(panel.querySelector('h2')?.textContent).toBe('backlinks.outgoingTitle');
    const outgoingButton = panel.querySelector<HTMLElement>('.q-sidebar-document-item')!;
    expect(panel.querySelector('.q-sidebar-document-item__title')?.textContent).toBe('Target');

    act(() => {
      outgoingButton.dispatchEvent(new MouseEvent('click', { bubbles: true, ctrlKey: true }));
    });
    expect(onOpenOutgoing).toHaveBeenCalledWith(linkMocks.outgoing, 'new-tab');
    expect(onOpenBacklink).not.toHaveBeenCalled();
  });
});
