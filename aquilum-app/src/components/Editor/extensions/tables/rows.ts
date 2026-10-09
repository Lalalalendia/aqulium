export const SEPARATOR_CELL = /^\s*:?-+:?\s*$/;

export function splitTableRow(line: string): string[] | null {
  const trimmed = line.trim();
  if (!trimmed.includes('|')) return null;
  const body = trimmed.replace(/^\|/, '').replace(/\|$/, '');
  const cells = body.split('|').map((cell) => cell.trim());
  return cells.length > 0 ? cells : null;
}

export function isSeparatorRow(line: string): boolean {
  const cells = splitTableRow(line);
  if (!cells?.length) return false;
  return cells.every((cell) => SEPARATOR_CELL.test(cell) && cell.replace(/\s/g, '').length > 0);
}
