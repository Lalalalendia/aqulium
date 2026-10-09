use super::analyzer::indexing_tokenizer;
use super::headings;
use super::index::SearchFields;
use super::models::SearchResult;
use std::path::Path;
use tantivy::schema::Value;
use tantivy::tokenizer::TokenStream;
use tantivy::TantivyDocument;
use unicode_normalization::char::{decompose_canonical, is_combining_mark};

const CHARS_BEFORE_MATCH: usize = 72;
const CHARS_AFTER_MATCH: usize = 188;

#[cfg(test)]
pub fn query_terms(input: &str) -> Vec<String> {
    let mut terms = input
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(normalize)
        .collect::<Vec<_>>();
    terms.sort();
    terms.dedup();
    terms
}

pub fn analyzed_query_terms(input: &str) -> Vec<String> {
    let mut terms = Vec::new();
    indexing_tokenizer().token_stream(input).process(&mut |token| {
        if !token.text.is_empty() {
            terms.push(token.text.clone());
        }
    });
    terms.sort();
    terms.dedup();
    terms
}

pub fn result_from_document(
    document: &TantivyDocument,
    fields: SearchFields,
    terms: &[String],
) -> Option<SearchResult> {
    let path = stored_text(document, fields.id)?;
    let title = stored_text(document, fields.title).unwrap_or_default();
    let body = stored_text(document, fields.body).unwrap_or_default();
    let file_name = Path::new(&path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&title);
    let body_matches = find_prefix_matches(&body, terms);
    let name_matches = find_prefix_matches(file_name, terms);
    let first_body_match = body_matches.first().map(|range| range.0);
    let match_count = name_matches.len() + body_matches.len();
    let match_offset = first_body_match
        .map(|offset| body[..offset].encode_utf16().count())
        .unwrap_or(0);
    let extension = Path::new(&path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_owned();
    Some(SearchResult {
        path,
        title,
        extension,
        snippet: make_excerpt(&body, first_body_match),
        match_count,
        match_offset,
        heading: first_body_match.and_then(|offset| {
            headings::enclosing(&headings::headings(&body), offset)
                .map(|heading| heading.title.clone())
        }),
        matched_terms: matched_terms(terms, &name_matches, &body_matches),
    })
}

pub fn find_prefix_matches(text: &str, terms: &[String]) -> Vec<(usize, usize, usize)> {
    let normalized = NormalizedText::new(text);
    let mut matches = Vec::new();
    for (index, term) in terms.iter().enumerate() {
        let mut from = 0;
        while from < normalized.value.len() {
            let Some(relative) = normalized.value[from..].find(term) else {
                break;
            };
            let start = from + relative;
            let end = start + term.len();
            let is_word_start = normalized.value[..start]
                .chars()
                .next_back()
                .is_none_or(|character| !character.is_alphanumeric());
            if is_word_start {
                if let Some(original) = normalized.original_range(start, end) {
                    matches.push((original.0, original.1, index));
                }
            }
            from = next_char_boundary(&normalized.value, start);
        }
    }
    matches.sort_unstable_by_key(|range| range.0);
    matches.dedup_by_key(|range| range.0);
    matches
}

fn matched_terms(
    terms: &[String],
    name_matches: &[(usize, usize, usize)],
    body_matches: &[(usize, usize, usize)],
) -> Vec<String> {
    let mut hit = vec![false; terms.len()];
    for (_, _, index) in name_matches.iter().chain(body_matches) {
        hit[*index] = true;
    }
    terms
        .iter()
        .zip(hit)
        .filter(|(_, found)| *found)
        .map(|(term, _)| term.clone())
        .collect()
}

pub fn make_excerpt(body: &str, match_start: Option<usize>) -> String {
    if body.is_empty() {
        return String::new();
    }
    let match_char = match_start
        .map(|offset| body[..offset].chars().count())
        .unwrap_or(0);
    let total_chars = body.chars().count();
    let start_char = match_char.saturating_sub(CHARS_BEFORE_MATCH);
    let end_char = (match_char + CHARS_AFTER_MATCH).min(total_chars);
    let excerpt = body[char_to_byte(body, start_char)..char_to_byte(body, end_char)]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{}{}{}",
        if start_char > 0 { "..." } else { "" },
        excerpt,
        if end_char < total_chars { "..." } else { "" }
    )
}

#[cfg(test)]
fn normalize(value: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    value
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .nfd()
        .filter(|character| !is_combining_mark(*character))
        .collect()
}

fn stored_text(document: &TantivyDocument, field: tantivy::schema::Field) -> Option<String> {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

fn next_char_boundary(text: &str, start: usize) -> usize {
    ((start + 1)..=text.len())
        .find(|offset| text.is_char_boundary(*offset))
        .unwrap_or(text.len())
}

fn char_to_byte(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}


// An ordinary normalized scalar with an unchanged UTF-8 byte width needs
// no per-character mapping: all offsets in the surrounding interval differ
// by one constant shift. This explicitly stores exceptions only.
#[derive(Clone, Copy)]
struct OffsetException {
    normalized_start: usize,
    normalized_end: usize,
    original_start: usize,
    original_end: usize,
}

struct NormalizedText {
    value: String,
    exceptions: Vec<OffsetException>,
}

impl NormalizedText {
    fn new(value: &str) -> Self {
        let mut normalized = String::with_capacity(value.len());
        let mut exceptions = Vec::new();
        for (original_start, character) in value.char_indices() {
            let original_end = original_start + character.len_utf8();
            let normalized_start = normalized.len();
            let mut emitted = 0usize;
            for lowered in character.to_lowercase() {
                decompose_canonical(lowered, |folded| {
                    if !is_combining_mark(folded) {
                        normalized.push(folded);
                        emitted += 1;
                    }
                });
            }
            let normalized_end = normalized.len();
            // A one-to-one scalar with equal byte width has a one-to-one
            // offset mapping, even if case-folding changed the glyph.
            if emitted != 1 || normalized_end - normalized_start != original_end - original_start {
                exceptions.push(OffsetException {
                    normalized_start,
                    normalized_end,
                    original_start,
                    original_end,
                });
            }
        }
        Self { value: normalized, exceptions }
    }

    fn span_at(&self, offset: usize) -> Option<(usize, usize)> {
        if offset >= self.value.len() { return None; }
        // Skip exceptional original scalars that normalized to zero bytes too.
        let index = self.exceptions.partition_point(|span| span.normalized_end <= offset);
        if let Some(exception) = self.exceptions.get(index) {
            if exception.normalized_start <= offset {
                return Some((exception.original_start, exception.original_end));
            }
        }

        // Between exceptional scalars, normalized and original UTF-8 offsets
        // differ by the constant shift established by the previous exception.
        let (previous_original_end, previous_normalized_end) = if index > 0 {
            let previous = &self.exceptions[index - 1];
            (previous.original_end, previous.normalized_end)
        } else { (0, 0) };

        let mut scalar_start = offset;
        while !self.value.is_char_boundary(scalar_start) {
            scalar_start -= 1;
        }
        let scalar_end = scalar_start + self.value[scalar_start..].chars().next()?.len_utf8();

        let (start, end) = if previous_original_end >= previous_normalized_end {
            let delta = previous_original_end - previous_normalized_end;
            (scalar_start.checked_add(delta)?, scalar_end.checked_add(delta)?)
        } else {
            let delta = previous_normalized_end - previous_original_end;
            (scalar_start.checked_sub(delta)?, scalar_end.checked_sub(delta)?)
        };
        Some((start, end))
    }

    fn original_range(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let first = self.span_at(start)?;
        let last = self.span_at(end.saturating_sub(1))?;
        Some((first.0, last.1))
    }
}

#[cfg(all(test, windows))]
#[path = "matching_sparse_tests.rs"]
mod sparse_tests;
