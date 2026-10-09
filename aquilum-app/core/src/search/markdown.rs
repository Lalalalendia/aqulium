pub fn lines_outside_fences(text: &str) -> Vec<(usize, &str)> {
    let mut kept = Vec::new();
    let mut fence: Option<String> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(opening) = &fence {
            if trimmed.starts_with(opening.as_str()) {
                fence = None;
            }
            continue;
        }
        if let Some(opening) = fence_marker(trimmed) {
            fence = Some(opening);
            continue;
        }
        kept.push((index, line));
    }
    kept
}

pub fn list_item_body(trimmed: &str) -> Option<&str> {
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return Some(rest.trim_start());
    }
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = &trimmed[digits..];
    rest.strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))
        .map(str::trim_start)
}

fn fence_marker(trimmed: &str) -> Option<String> {
    for symbol in ['`', '~'] {
        let length = trimmed.chars().take_while(|found| *found == symbol).count();
        if length >= 3 {
            return Some(std::iter::repeat_n(symbol, length).collect());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{lines_outside_fences, list_item_body};

    #[test]
    fn only_dash_and_numbered_markers_start_a_list_item() {
        assert_eq!(list_item_body("- пункт"), Some("пункт"));
        assert_eq!(list_item_body("12. пункт"), Some("пункт"));
        assert_eq!(list_item_body("3) пункт"), Some("пункт"));
        assert_eq!(list_item_body("* пункт"), None);
        assert_eq!(list_item_body("+ пункт"), None);
        assert_eq!(list_item_body("-пункт"), None);
        assert_eq!(list_item_body("3.пункт"), None);
    }

    #[test]
    fn a_fence_hides_its_lines_until_the_same_marker_closes_it() {
        let text = "до\n````md\n```\nвнутри\n````\nпосле\n~~~\nтильды\n~~~";
        assert_eq!(lines_outside_fences(text), vec![(0, "до"), (5, "после")]);
    }
}
