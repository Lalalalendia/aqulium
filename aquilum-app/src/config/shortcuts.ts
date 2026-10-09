export interface ShortcutToken {
  code?: string;
  key: string;
  display?: string;
  primary?: boolean;
  shift?: boolean;
  alt?: boolean;
}

export const SHORTCUTS = {
  NEW_FILE: {
    code: 'KeyN',
    key: 'N',
    primary: true,
  },
  GLOBAL_SEARCH: {
    code: 'KeyO',
    key: 'O',
    primary: true,
  },
  NEW_FROM_TEMPLATE: {
    code: 'KeyU',
    key: 'U',
    primary: true,
  },
  FOCUS_MODE: {
    code: 'KeyF',
    key: 'F',
    primary: true,
    shift: true,
  },
  PAGE_SEARCH: {
    code: 'KeyF',
    key: 'F',
    primary: true,
  },
  ZOOM_IN: {
    key: '=',
    display: '+',
    primary: true,
  },
  ZOOM_OUT: {
    key: '-',
    primary: true,
  },
  ZOOM_RESET: {
    key: '0',
    primary: true,
  },
  BOLD: {
    key: 'B',
    primary: true,
  },
  ITALIC: {
    key: 'I',
    primary: true,
  },
  STRIKETHROUGH: {
    key: 'S',
    primary: true,
    shift: true,
  },
  INDENT: {
    key: 'Tab',
  },
  OUTDENT: {
    key: 'Tab',
    shift: true,
  },
  LINE_BREAK: {
    key: 'Enter',
    display: '↵',
    shift: true,
  },
  PAGE_SEARCH_NEXT: {
    key: 'Enter',
    display: '↵',
  },
  PAGE_SEARCH_PREVIOUS: {
    key: 'Enter',
    display: '↵',
    shift: true,
  },
  READER_NEXT_PAGE: {
    key: 'ArrowRight',
    display: '→',
  },
  READER_PREVIOUS_PAGE: {
    key: 'ArrowLeft',
    display: '←',
  },
  SEARCH_PREVIOUS: {
    key: 'ArrowUp',
    display: '↑',
  },
  SEARCH_NEXT: {
    key: 'ArrowDown',
    display: '↓',
  },
  SEARCH_OPEN: {
    key: 'Enter',
    display: '↵',
  },
  CLOSE_DIALOG: {
    key: 'Escape',
    display: 'Esc',
  },
  SEARCH_OPEN_NEW_PANE: {
    key: 'Enter',
    display: '↵',
    primary: true,
  },
  SEARCH_CREATE: {
    key: 'Enter',
    display: '↵',
    shift: true,
  },
} as const satisfies Record<string, ShortcutToken>;

export const ZOOM_IN_ALIASES: readonly ShortcutToken[] = [
  { key: '+', primary: true },
  { key: '+', primary: true, shift: true },
];

export function matchesShortcut(event: KeyboardEvent, shortcut: ShortcutToken): boolean {
  const hasPrimaryModifier = event.ctrlKey || event.metaKey;
  const keyMatches = shortcut.code
    ? event.code === shortcut.code
    : event.key === shortcut.key;
  return keyMatches
    && hasPrimaryModifier === Boolean(shortcut.primary)
    && event.altKey === Boolean(shortcut.alt)
    && event.shiftKey === Boolean(shortcut.shift);
}

export function codeMirrorKey(shortcut: ShortcutToken): string {
  const keys = [];
  if (shortcut.primary) keys.push('Mod');
  if (shortcut.alt) keys.push('Alt');
  if (shortcut.shift) keys.push('Shift');
  keys.push(shortcut.key.length === 1 ? shortcut.key.toLowerCase() : shortcut.key);
  return keys.join('-');
}

export function shortcutModifiers(shortcut: ShortcutToken): string[] {
  const keys = [];
  if (shortcut.primary) keys.push('Ctrl');
  if (shortcut.shift) keys.push('Shift');
  if (shortcut.alt) keys.push('Alt');
  return keys;
}

export function formatShortcut(shortcut: ShortcutToken, separator = ' + '): string {
  return [...shortcutModifiers(shortcut), shortcut.display ?? shortcut.key].join(separator);
}
