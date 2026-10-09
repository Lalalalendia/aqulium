//! Correct coordinate conversion at the CodeMirror/Yrs UTF-16 <-> Rust UTF-8 seam.
//! A UTF-16 offset in the middle of a surrogate pair is INVALID. Clamping it
//! would silently edit a different character, so report an explicit error.
//! All returned byte offsets are valid Rust UTF-8 boundaries.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionError {
    OutOfBounds,
    InsideUtf16Surrogate,
    InsideUtf8Codepoint,
    ReversedRange,
}

pub fn utf16_to_byte(text: &str, offset: usize) -> Result<usize, PositionError> {
    if offset == 0 { return Ok(0); }
    let mut utf16 = 0usize;
    for (at, ch) in text.char_indices() {
        let end = utf16 + ch.len_utf16();
        if offset == end {
            return Ok(at + ch.len_utf8());
        }
        if offset < end {
            return Err(PositionError::InsideUtf16Surrogate);
        }
        utf16 = end;
    }
    Err(PositionError::OutOfBounds)
}

pub fn byte_to_utf16(text: &str, offset: usize) -> Result<usize, PositionError> {
    if offset > text.len() { return Err(PositionError::OutOfBounds); }
    if !text.is_char_boundary(offset) { return Err(PositionError::InsideUtf8Codepoint); }
    let mut count = 0;
    for (at, ch) in text.char_indices() {
        if at == offset { return Ok(count); }
        count += ch.len_utf16();
    }
    Ok(count)
}

pub fn range_utf16_to_bytes(text: &str, start: usize, end: usize)
    -> Result<std::ops::Range<usize>, PositionError>
{
    if start > end { return Err(PositionError::ReversedRange); }
    Ok(utf16_to_byte(text, start)?..utf16_to_byte(text, end)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_round_trips_all_unicode_character_boundaries() {
        for text in [
            "",
            "ascii\n",
            "Привет, мир!",
            "A🌍B👍🏼C",
            "🇷🇺\u{200d}X",
            "e\u{0301} cafe\u{0301}\r\nZ",
            "日本語\u{200d}✈️\t",
        ] {
            let mut units = 0;
            for (at, ch) in text.char_indices() {
                assert_eq!(byte_to_utf16(text, at), Ok(units), "{text:?} {at}");
                assert_eq!(utf16_to_byte(text, units), Ok(at), "{text:?} {units}");
                units += ch.len_utf16();
            }
            assert_eq!(byte_to_utf16(text, text.len()), Ok(units));
            assert_eq!(utf16_to_byte(text, units), Ok(text.len()));
        }
    }

    #[test]
    fn rejects_surrogate_splitting_and_invalid_utf8_byte_boundaries() {
        let text = "a🌍б";
        assert_eq!(utf16_to_byte(text, 2), Err(PositionError::InsideUtf16Surrogate));
        assert_eq!(utf16_to_byte(text, 5), Err(PositionError::OutOfBounds));
        assert_eq!(byte_to_utf16(text, 2), Err(PositionError::InsideUtf8Codepoint));
        assert_eq!(byte_to_utf16(text, 8), Err(PositionError::OutOfBounds));
        assert_eq!(range_utf16_to_bytes(text, 3, 1), Err(PositionError::ReversedRange));
    }

    #[test]
    fn byte_edit_range_preserves_exact_russian_and_emoji_content() {
        let mut original = String::from("Старт 🌍 конец\r\n");
        let range = range_utf16_to_bytes(&original, 6, 8).unwrap();
        assert_eq!(&original[range.clone()], "🌍");
        original.replace_range(range, "👍🏼");
        assert_eq!(original, "Старт 👍🏼 конец\r\n");
    }

    #[test]
    fn permits_combining_sequence_partial_edits_without_corrupting_utf8() {
        let text = "e\u{0301}Z";
        let range = range_utf16_to_bytes(text, 1, 2).unwrap();
        assert_eq!(&text[range], "\u{0301}");
        // Grapheme semantics remain an editor concern; codepoints are valid.
    }
}
