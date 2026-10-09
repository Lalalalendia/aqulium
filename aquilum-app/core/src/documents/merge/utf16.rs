pub fn utf16_len(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

pub fn utf16_slice(text: &str, from: usize, to: usize) -> String {
    let mut position = 0;
    let mut result = String::new();
    for character in text.chars() {
        if position >= to {
            break;
        }
        if position >= from {
            result.push(character);
        }
        position += character.len_utf16();
    }
    result
}
