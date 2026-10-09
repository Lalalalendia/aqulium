/**
 * Exact same narrow kernel as QuoteScan.res and rust/src/lib.rs.
 * Line-oriented Markdown callout-header prefilter adapted from Aquilum's
 * quoteScanPresence initial scan. This does NOT parse full book/reader quotes.
 * Return (bookCount * 65536 + quoteCount); fixtures stay below 65536 each.
 */
function asciiPrefixEqual(text: string, at: number, word: string): boolean {
  if (at + word.length > text.length) return false;
  for (let i = 0; i < word.length; i++) {
    const c = text.charCodeAt(at + i);
    const normalized = c >= 65 && c <= 90 ? c + 32 : c;
    if (normalized !== word.charCodeAt(i)) return false;
  }
  return true;
}

export function scanMarkers(text: string): number {
  const n = text.length;
  let lineFrom = 0;
  let books = 0;
  let quotes = 0;
  while (lineFrom < n) {
    let at = lineFrom;
    while (at < n && (text.charCodeAt(at) === 32 || text.charCodeAt(at) === 9)) at++;
    if (at < n && text.charCodeAt(at) === 62) {
      at++;
      while (at < n && (text.charCodeAt(at) === 32 || text.charCodeAt(at) === 9)) at++;
      if (asciiPrefixEqual(text, at, "[!book]")) books++;
      else if (asciiPrefixEqual(text, at, "[!quote]")) quotes++;
    }
    const newline = text.indexOf("\n", lineFrom);
    lineFrom = newline === -1 ? n : newline + 1;
  }
  if (books > 65535 || quotes > 65535) throw new Error("Benchmark marker count overflow");
  return books * 65536 + quotes;
}
