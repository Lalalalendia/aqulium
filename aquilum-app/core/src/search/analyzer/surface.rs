use super::indexing_tokenizer;
use std::collections::HashMap;
use tantivy::tokenizer::TokenStream;

pub fn stem_surface_labels(title: &str, body: &str) -> HashMap<String, String> {
    let mut by_stem: HashMap<String, HashMap<String, u32>> = HashMap::new();
    for text in [title, body] {
        for surface in alphanumeric_words(text) {
            if surface.chars().count() < 2 {
                continue;
            }
            let Some(stem) = stem_token(surface) else {
                continue;
            };
            if stem.chars().count() < 2 {
                continue;
            }
            *by_stem
                .entry(stem)
                .or_default()
                .entry(surface.to_owned())
                .or_insert(0) += 1;
        }
    }
    by_stem
        .into_iter()
        .filter_map(|(stem, surfaces)| {
            surfaces
                .into_iter()
                .max_by(|(left, left_count), (right, right_count)| {
                    left_count
                        .cmp(right_count)
                        .then_with(|| right.chars().count().cmp(&left.chars().count()))
                        .then_with(|| right.cmp(left))
                })
                .map(|(surface, _)| (stem, surface))
        })
        .collect()
}

fn stem_token(surface: &str) -> Option<String> {
    let mut stem = None;
    indexing_tokenizer()
        .token_stream(surface)
        .process(&mut |token| {
            if stem.is_none() && !token.text.is_empty() {
                stem = Some(token.text.clone());
            }
        });
    stem
}

fn alphanumeric_words(text: &str) -> impl Iterator<Item = &str> {
    WordSpans { text, cursor: 0 }
}

struct WordSpans<'a> {
    text: &'a str,
    cursor: usize,
}

impl<'a> Iterator for WordSpans<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        while self.cursor < self.text.len() {
            let ch = self.text[self.cursor..].chars().next()?;
            if ch.is_alphanumeric() {
                break;
            }
            self.cursor += ch.len_utf8();
        }
        if self.cursor >= self.text.len() {
            return None;
        }
        let start = self.cursor;
        while self.cursor < self.text.len() {
            let ch = self.text[self.cursor..].chars().next()?;
            if !ch.is_alphanumeric() {
                break;
            }
            self.cursor += ch.len_utf8();
        }
        Some(&self.text[start..self.cursor])
    }
}

#[cfg(test)]
mod tests {
    use super::stem_surface_labels;

    #[test]
    fn maps_russian_stems_to_surface_words() {
        let labels = stem_surface_labels("новая книга", "про новую книгу");
        assert_eq!(labels.get("книг").map(String::as_str), Some("книга"));
        assert_eq!(labels.get("нов").map(String::as_str), Some("новая"));
    }

    #[test]
    fn prefers_the_most_frequent_surface_form() {
        let labels = stem_surface_labels("", "книгу книгу книгу книга");
        assert_eq!(labels.get("книг").map(String::as_str), Some("книгу"));
    }

    #[test]
    fn maps_english_stems_to_surface_words() {
        let labels = stem_surface_labels("machines", "about machines and machine");
        assert_eq!(labels.get("machin").map(String::as_str), Some("machines"));
    }
}
