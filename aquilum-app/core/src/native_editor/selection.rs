use super::rope::RopeBuffer;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
    pub preferred_column: Option<usize>,
}

impl Selection {
    pub fn caret(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
            preferred_column: None,
        }
    }

    pub fn new(anchor: usize, head: usize) -> Self {
        Self {
            anchor,
            head,
            preferred_column: None,
        }
    }

    pub fn with_preferred_column(mut self, col: Option<usize>) -> Self {
        self.preferred_column = col;
        self
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }

    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    pub fn clamp(&self, max_chars: usize) -> Self {
        Self {
            anchor: self.anchor.min(max_chars),
            head: self.head.min(max_chars),
            preferred_column: self.preferred_column,
        }
    }

    /// Maps the selection through an external edit covering `[from_char..to_char]`
    /// with `inserted_chars` new characters.
    pub fn map_through_edit(
        &self,
        from_char: usize,
        to_char: usize,
        inserted_chars: usize,
        max_chars: usize,
    ) -> Self {
        let map_point = |pt: usize| -> usize {
            if pt <= from_char {
                pt
            } else if pt >= to_char {
                pt - (to_char - from_char) + inserted_chars
            } else {
                from_char + inserted_chars
            }
        };
        Self {
            anchor: map_point(self.anchor).min(max_chars),
            head: map_point(self.head).min(max_chars),
            preferred_column: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Newline,
    Whitespace,
    Word,
    Punctuation,
}

fn classify(ch: char) -> CharClass {
    if ch == '\n' || ch == '\r' {
        CharClass::Newline
    } else if ch.is_whitespace() {
        CharClass::Whitespace
    } else if ch.is_alphanumeric() || ch == '_' {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

pub fn prev_word_boundary(rope: &RopeBuffer, from_char: usize) -> usize {
    let mut idx = from_char.min(rope.len_chars());
    if idx == 0 {
        return 0;
    }
    // Step back over horizontal whitespace first
    while idx > 0 {
        let Some(ch) = rope.char_at(idx - 1) else { break };
        if classify(ch) == CharClass::Whitespace {
            idx -= 1;
        } else {
            break;
        }
    }
    if idx == 0 {
        return 0;
    }
    let Some(first_ch) = rope.char_at(idx - 1) else { return 0 };
    let target_class = classify(first_ch);
    idx -= 1;
    if target_class == CharClass::Newline {
        return idx;
    }
    while idx > 0 {
        let Some(ch) = rope.char_at(idx - 1) else { break };
        if classify(ch) == target_class {
            idx -= 1;
        } else {
            break;
        }
    }
    idx
}

pub fn next_word_boundary(rope: &RopeBuffer, from_char: usize) -> usize {
    let len = rope.len_chars();
    let mut idx = from_char.min(len);
    if idx >= len {
        return len;
    }
    // Step over horizontal whitespace first
    while idx < len {
        let Some(ch) = rope.char_at(idx) else { break };
        if classify(ch) == CharClass::Whitespace {
            idx += 1;
        } else {
            break;
        }
    }
    if idx >= len {
        return len;
    }
    let Some(first_ch) = rope.char_at(idx) else { return len };
    let target_class = classify(first_ch);
    idx += 1;
    if target_class == CharClass::Newline {
        return idx;
    }
    while idx < len {
        let Some(ch) = rope.char_at(idx) else { break };
        if classify(ch) == target_class {
            idx += 1;
        } else {
            break;
        }
    }
    idx
}
