//! One shared Rust scanner compiled into both native Rust and WebAssembly.
//! Algorithm matches the lexical prefilter in ../../src/scan.ts.
pub fn scan_bytes(bytes: &[u8]) -> u32 {
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
    assert!(books <= 65535 && quotes <= 65535, "benchmark marker count overflow");
    (books << 16) | quotes
}
