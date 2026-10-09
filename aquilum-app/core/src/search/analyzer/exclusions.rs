const EXCLUDED_TERMS: &[&str] = &[
    "com", "htm", "html", "http", "https", "net", "org", "ru", "www",
];

pub fn is_excluded_term(term: &str) -> bool {
    EXCLUDED_TERMS.binary_search(&term).is_ok()
}

#[cfg(test)]
mod tests {
    use super::is_excluded_term;

    #[test]
    fn excludes_url_noise_terms() {
        assert!(is_excluded_term("https"));
        assert!(is_excluded_term("http"));
        assert!(is_excluded_term("www"));
        assert!(is_excluded_term("ru"));
        assert!(is_excluded_term("com"));
    }

    #[test]
    fn keeps_normal_words_including_short_ones() {
        assert!(!is_excluded_term("rust"));
        assert!(!is_excluded_term("нейросеть"));
        assert!(!is_excluded_term("note"));
        assert!(!is_excluded_term("de"));
        assert!(!is_excluded_term("me"));
        assert!(!is_excluded_term("us"));
        assert!(!is_excluded_term("co"));
    }
}
