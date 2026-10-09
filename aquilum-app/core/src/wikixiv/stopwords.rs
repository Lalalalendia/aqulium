use std::collections::HashSet;
use std::sync::LazyLock;

static STOPWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    include_str!("stopwords.txt")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
});

pub fn is_stop_term(term: &str) -> bool {
    STOPWORDS.contains(term)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_known_stopwords() {
        assert!(is_stop_term("the"));
        assert!(is_stop_term("для"));
        assert!(is_stop_term("управляет"));
        assert!(is_stop_term("доступен"));
        assert!(is_stop_term("несколько"));
        assert!(!is_stop_term("physics"));
        assert!(!is_stop_term("секция"));
        assert!(!is_stop_term("panel"));
        assert!(!is_stop_term("accordion"));
    }
}
