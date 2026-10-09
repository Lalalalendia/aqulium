/* Real ReScript 12 implementation of the Aquilum lexical quote-header prefilter.
   No JS external is used for the algorithm: this is not a TypeScript wrapper. */

let asciiPrefixEqual = (text: string, at: int, word: string, textSize: int): bool => {
  let wordSize = String.length(word)
  if at + wordSize > textSize {
    false
  } else {
    let good = ref(true)
    let offset = ref(0)
    while offset.contents < wordSize && good.contents {
      let ch = String.charCodeAtUnsafe(text, at + offset.contents)
      let norm = if ch >= 65 && ch <= 90 {ch + 32} else {ch}
      if norm != String.charCodeAtUnsafe(word, offset.contents) {
        good := false
      }
      offset := offset.contents + 1
    }
    good.contents
  }
}

let scanMarkers = (text: string): int => {
  let total = String.length(text)
  let lineFrom = ref(0)
  let books = ref(0)
  let quotes = ref(0)
  while lineFrom.contents < total {
    let at = ref(lineFrom.contents)
    while at.contents < total &&
      (String.charCodeAtUnsafe(text, at.contents) == 32 ||
        String.charCodeAtUnsafe(text, at.contents) == 9) {
      at := at.contents + 1
    }
    if at.contents < total && String.charCodeAtUnsafe(text, at.contents) == 62 {
      at := at.contents + 1
      while at.contents < total &&
        (String.charCodeAtUnsafe(text, at.contents) == 32 ||
          String.charCodeAtUnsafe(text, at.contents) == 9) {
        at := at.contents + 1
      }
      if asciiPrefixEqual(text, at.contents, "[!book]", total) {
        books := books.contents + 1
      } else if asciiPrefixEqual(text, at.contents, "[!quote]", total) {
        quotes := quotes.contents + 1
      }
    }
    let newline = String.indexOfFrom(text, "\n", lineFrom.contents)
    lineFrom := if newline == -1 {total} else {newline + 1}
  }
  books.contents * 65536 + quotes.contents
}
