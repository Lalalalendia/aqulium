import { describe, expect, it } from 'vitest';
import { codeMirrorKey, matchesShortcut, SHORTCUTS } from './shortcuts';

describe('global shortcut tokens', () => {
  it('matches physical keys independently of the active keyboard layout', () => {
    const russianLayoutEvent = {
      code: 'KeyO',
      key: 'щ',
      ctrlKey: true,
      metaKey: false,
      altKey: false,
      shiftKey: false,
    } as KeyboardEvent;

    expect(matchesShortcut(russianLayoutEvent, SHORTCUTS.GLOBAL_SEARCH)).toBe(true);
  });

  it('does not consume modified variants of a shortcut', () => {
    const shiftedEvent = {
      code: 'KeyO',
      key: 'O',
      ctrlKey: true,
      metaKey: false,
      altKey: false,
      shiftKey: true,
    } as KeyboardEvent;

    expect(matchesShortcut(shiftedEvent, SHORTCUTS.GLOBAL_SEARCH)).toBe(false);
  });

  it('matches search command variants', () => {
    const event = (key: string, modifiers: Partial<KeyboardEvent> = {}) => ({
      key,
      ctrlKey: false,
      metaKey: false,
      altKey: false,
      shiftKey: false,
      ...modifiers,
    } as KeyboardEvent);

    expect(matchesShortcut(event('ArrowUp'), SHORTCUTS.SEARCH_PREVIOUS)).toBe(true);
    expect(matchesShortcut(event('Enter'), SHORTCUTS.SEARCH_OPEN)).toBe(true);
    expect(matchesShortcut(event('Enter', { ctrlKey: true }), SHORTCUTS.SEARCH_OPEN_NEW_PANE)).toBe(true);
    expect(matchesShortcut(event('Enter', { shiftKey: true }), SHORTCUTS.SEARCH_CREATE)).toBe(true);
    expect(matchesShortcut(event('Enter', { ctrlKey: true, shiftKey: true }), SHORTCUTS.SEARCH_CREATE)).toBe(false);
  });

  it('spells editor shortcuts the way CodeMirror keymaps expect', () => {
    expect(codeMirrorKey(SHORTCUTS.BOLD)).toBe('Mod-b');
    expect(codeMirrorKey(SHORTCUTS.STRIKETHROUGH)).toBe('Mod-Shift-s');
    expect(codeMirrorKey(SHORTCUTS.OUTDENT)).toBe('Shift-Tab');
    expect(codeMirrorKey(SHORTCUTS.LINE_BREAK)).toBe('Shift-Enter');
  });

  it('keeps focus mode apart from the page search', () => {
    const event = {
      code: 'KeyF',
      key: 'F',
      ctrlKey: true,
      metaKey: false,
      altKey: false,
      shiftKey: true,
    } as KeyboardEvent;

    expect(matchesShortcut(event, SHORTCUTS.FOCUS_MODE)).toBe(true);
    expect(matchesShortcut(event, SHORTCUTS.PAGE_SEARCH)).toBe(false);
  });
});
