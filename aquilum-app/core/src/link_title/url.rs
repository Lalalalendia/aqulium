pub fn normalize(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty url".into());
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }
    if trimmed.starts_with("www.") {
        return Ok(format!("https://{trimmed}"));
    }
    Err("unsupported url scheme".into())
}

pub fn hostname_fallback(url: &str) -> String {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(without_scheme);
    let host = host.split('@').next_back().unwrap_or(host);
    if host.is_empty() {
        url.to_string()
    } else {
        host.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{hostname_fallback, normalize};

    #[test]
    fn hostname_from_url() {
        assert_eq!(hostname_fallback("https://example.com/path"), "example.com");
    }

    #[test]
    fn hostname_drops_credentials_and_query() {
        assert_eq!(hostname_fallback("https://user@example.com/a?b=1"), "example.com");
    }

    #[test]
    fn a_bare_www_address_gets_a_scheme() {
        assert_eq!(normalize(" www.example.com "), Ok("https://www.example.com".into()));
    }

    #[test]
    fn an_address_that_already_has_a_scheme_is_kept() {
        assert_eq!(normalize("http://example.com"), Ok("http://example.com".into()));
    }

    #[test]
    fn refuses_what_is_not_a_web_address() {
        assert!(normalize("").is_err());
        assert!(normalize("file:///etc/passwd").is_err());
    }
}
