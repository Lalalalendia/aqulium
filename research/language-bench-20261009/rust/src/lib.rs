use wasm_bindgen::prelude::*;

/// Same lexical prefilter as scan.ts and QuoteScan.res.
///
/// Checks ASCII spaces/tabs before '>', spaces/tabs after it, and ASCII
/// case-insensitive [!book] or [!quote] prefix. Does not parse rich quotes.
/// UTF-8 bytes are safe here: only ASCII syntax is inspected. JS->Wasm
/// UTF-8 encoding + memory copy IS INCLUDED in JS-call benchmarks.
#[wasm_bindgen]
pub fn scan_markers(input: &str) -> u32 {
    let bytes = input.as_bytes();
    let mut line_from = 0usize;
    let mut books = 0u32;
    let mut quotes = 0u32;
    while line_from < bytes.len() {
        let mut at = line_from;
        while at < bytes.len() && (bytes[at] == b' ' || bytes[at] == b'\t') {
            at += 1;
        }
        if at < bytes.len() && bytes[at] == b'>' {
            at += 1;
            while at < bytes.len() && (bytes[at] == b' ' || bytes[at] == b'\t') {
                at += 1;
            }
            if bytes
                .get(at..at.saturating_add(7))
                .is_some_and(|s| s.eq_ignore_ascii_case(b"[!book]"))
            {
                books += 1;
            } else if bytes
                .get(at..at.saturating_add(8))
                .is_some_and(|s| s.eq_ignore_ascii_case(b"[!quote]"))
            {
                quotes += 1;
            }
        }
        line_from = match memchr::memchr(b'\n', &bytes[line_from..]) {
            Some(relative) => line_from + relative + 1,
            None => bytes.len(),
        };
    }
    assert!(books <= 65535 && quotes <= 65535, "benchmark marker overflow");
    (books << 16) | quotes
}
