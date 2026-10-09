use super::frontmatter::strip_frontmatter;

pub fn sanitize_for_search(text: &str) -> String {
    sanitize_body(&strip_frontmatter(text))
}

fn sanitize_body(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    let bytes = text.as_bytes();
    while cursor < bytes.len() {
        if bytes[cursor] == b'[' {
            if let Some((label, next)) = take_markdown_link(text, cursor) {
                if !label.is_empty() {
                    if !out.is_empty() && !out.ends_with(char::is_whitespace) {
                        out.push(' ');
                    }
                    out.push_str(label);
                }
                cursor = next;
                continue;
            }
        }
        if let Some(next) = take_bare_url(text, cursor) {
            if !out.is_empty() && !out.ends_with(char::is_whitespace) {
                out.push(' ');
            }
            cursor = next;
            continue;
        }
        let ch = text[cursor..].chars().next().expect("cursor in bound");
        out.push(ch);
        cursor += ch.len_utf8();
    }
    out
}

fn take_markdown_link(text: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start + 1) == Some(&b'[') {
        return None;
    }
    let close_label = text[start + 1..].find(']')? + start + 1;
    if bytes.get(close_label + 1) != Some(&b'(') {
        return None;
    }
    let url_start = close_label + 2;
    let close_url = find_balanced_paren(text, url_start)?;
    let label = text[start + 1..close_label].trim();
    Some((label, close_url + 1))
}

fn find_balanced_paren(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 1i32;
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'\\' if i + 1 < bytes.len() => i += 1,
            _ => {}
        }
        i += 1;
    }
    None
}

fn take_bare_url(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let rest = &bytes[start..];
    let prefix_len = if rest.len() >= 8 && rest[..8].eq_ignore_ascii_case(b"https://") {
        8
    } else if rest.len() >= 7 && rest[..7].eq_ignore_ascii_case(b"http://") {
        7
    } else if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case(b"www.") {
        4
    } else {
        return None;
    };
    let mut end = start + prefix_len;
    for ch in text[end..].chars() {
        if ch.is_whitespace() || matches!(ch, ')' | ']' | '(' | '[' | '"' | '\'' | '<' | '>' | '`') {
            break;
        }
        if matches!(ch, '.' | ',' | ';' | ':' | '!' | '?') {
            let next = text[end + ch.len_utf8()..]
                .chars()
                .next()
                .filter(|value| !value.is_whitespace() && value.is_alphanumeric());
            if next.is_none() {
                break;
            }
        }
        end += ch.len_utf8();
    }
    (end > start + prefix_len).then_some(end)
}

#[cfg(test)]
mod tests {
    use super::sanitize_for_search;

    #[test]
    fn keeps_markdown_link_label_and_drops_url() {
        let cleaned = sanitize_for_search("see [NeuroNet](https://example.ru/path?x=1) now");
        assert_eq!(cleaned, "see NeuroNet now");
        assert!(!cleaned.contains("https"));
        assert!(!cleaned.contains("example"));
        assert!(!cleaned.contains("ru"));
    }

    #[test]
    fn leaves_wiki_links_alone() {
        assert_eq!(
            sanitize_for_search("see [[NeuroNet]] and [[Path/Note|alias]]"),
            "see [[NeuroNet]] and [[Path/Note|alias]]"
        );
    }

    #[test]
    fn strips_bare_urls() {
        let cleaned = sanitize_for_search("open https://site.com/a and www.foo.org/x end");
        assert_eq!(cleaned, "open  and  end");
        assert!(!cleaned.contains("https"));
        assert!(!cleaned.contains("site"));
        assert!(!cleaned.contains("www"));
    }

    #[test]
    fn empty_label_drops_whole_markdown_link() {
        assert_eq!(sanitize_for_search("x [](https://a.ru) y"), "x  y");
    }
}
