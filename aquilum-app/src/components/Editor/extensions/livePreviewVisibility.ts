import type { Text } from '@codemirror/state';

export function shouldRevealSyntax(
  doc: Text,
  head: number,
  from: number,
  to: number,
): boolean {
  if (to < from) return false;
  const caret = Math.max(0, Math.min(head, doc.length));
  const caretLine = doc.lineAt(caret);
  const ownerOnCaretLine = to > caretLine.from && from < caretLine.to;
  return ownerOnCaretLine && caret >= from && caret <= to;
}
