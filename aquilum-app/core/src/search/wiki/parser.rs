use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use std::ops::Range;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WikiLink {
    pub target: String,
    pub target_range: Range<usize>,
    pub offset_utf16: usize,
}

pub fn extract(source: &str) -> Vec<WikiLink> {
    let excluded = excluded_ranges(source);
    let bytes = source.as_bytes();
    let mut ranges = Vec::new();
    let mut cursor = 0;
    let mut excluded_at = 0;
    while cursor + 3 < bytes.len() {
        if bytes[cursor] != b'[' || bytes[cursor + 1] != b'[' || escaped(bytes, cursor) {
            cursor += 1;
            continue;
        }
        let Some(end) = find_close(bytes, cursor + 2) else {
            cursor += 2;
            continue;
        };
        let full = cursor..end + 2;
        while excluded_at < excluded.len() && excluded[excluded_at].end <= full.start {
            excluded_at += 1;
        }
        if excluded
            .get(excluded_at)
            .is_some_and(|range| overlaps(range, &full))
        {
            cursor = end + 2;
            continue;
        }
        let separator = source[cursor + 2..end].find('|').map(|at| cursor + 2 + at);
        let raw_end = separator.unwrap_or(end);
        let raw = &source[cursor + 2..raw_end];
        let leading = raw.len() - raw.trim_start().len();
        let trailing = raw.len() - raw.trim_end().len();
        let target_range = cursor + 2 + leading..raw_end - trailing;
        if !target_range.is_empty() {
            ranges.push(target_range);
        }
        cursor = end + 2;
    }
    let mut prior = 0;
    let mut offset_utf16 = 0;
    ranges
        .into_iter()
        .map(|target_range| {
            let link_start = target_range.start - 2;
            offset_utf16 += source[prior..link_start].encode_utf16().count();
            prior = link_start;
            WikiLink {
                target: source[target_range.clone()].to_owned(),
                target_range,
                offset_utf16,
            }
        })
        .collect()
}

fn excluded_ranges(source: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut block_start = None;
    for (event, range) in Parser::new_ext(source, Options::all()).into_offset_iter() {
        match event {
            Event::Code(_) | Event::Html(_) | Event::InlineHtml(_) => ranges.push(range),
            Event::Start(Tag::CodeBlock(_)) => block_start = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = block_start.take() {
                    ranges.push(start..range.end);
                }
            }
            _ => {}
        }
    }
    ranges.sort_by_key(|range| range.start);
    ranges
}

fn find_close(bytes: &[u8], mut cursor: usize) -> Option<usize> {
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == b'\n' || bytes[cursor] == b'\r' {
            return None;
        }
        if bytes[cursor] == b']' && bytes[cursor + 1] == b']' && !escaped(bytes, cursor) {
            return Some(cursor);
        }
        cursor += 1;
    }
    None
}

fn escaped(bytes: &[u8], at: usize) -> bool {
    let mut count = 0;
    let mut cursor = at;
    while cursor > 0 && bytes[cursor - 1] == b'\\' {
        count += 1;
        cursor -= 1;
    }
    count % 2 == 1
}

fn overlaps(left: &Range<usize>, right: &Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn extracts_targets_and_ignores_code() {
        let source = "[[Note]] [[Folder/Name|alias]] `[[Code]]`\n```\n[[Block]]\n```";
        let links = extract(source);
        assert_eq!(
            links
                .iter()
                .map(|link| link.target.as_str())
                .collect::<Vec<_>>(),
            ["Note", "Folder/Name"]
        );
    }

    #[test]
    fn continues_after_an_unclosed_link() {
        let links = extract("[[broken\n[[Note]]");
        assert_eq!(links[0].target, "Note");
    }
}
