use super::entities;

pub const CLOSING_TAG: &str = "</title>";

fn collapse_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find("<title")?;
    let after_open = start + "<title".len();
    let gt = lower[after_open..].find('>')?;
    let content_start = after_open + gt + 1;
    let close_rel = lower[content_start..].find(CLOSING_TAG)?;
    let raw = html[content_start..content_start + close_rel].trim();
    if raw.is_empty() {
        return None;
    }
    let decoded = entities::decode(&collapse_ws(raw));
    let trimmed = decoded.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parses_simple_title() {
        let html = "<html><head><title>Hello World</title></head></html>";
        assert_eq!(parse(html).as_deref(), Some("Hello World"));
    }

    #[test]
    fn parses_title_case_insensitive_and_collapses_ws() {
        let html = "<HTML><TITLE>\n  Foo\t Bar  \n</TITLE></HTML>";
        assert_eq!(parse(html).as_deref(), Some("Foo Bar"));
    }

    #[test]
    fn reads_a_title_tag_that_carries_attributes() {
        let html = "<title lang=\"ru\">Заголовок</title>";
        assert_eq!(parse(html).as_deref(), Some("Заголовок"));
    }

    #[test]
    fn empty_title_is_none() {
        assert!(parse("<title>   </title>").is_none());
        assert!(parse("<div>no title</div>").is_none());
    }

    #[test]
    fn a_youtube_title_reads_like_a_sentence_not_like_html() {
        let html = "<title>We&#39;re Not Ready for Biocomputing - YouTube</title>";
        assert_eq!(
            parse(html).as_deref(),
            Some("We're Not Ready for Biocomputing - YouTube")
        );
    }

    #[test]
    fn a_title_made_only_of_entities_that_collapse_to_space_is_none() {
        assert!(parse("<title>&nbsp;&nbsp;</title>").is_none());
    }
}
