use super::index::SearchIndex;
use super::matching::{find_prefix_matches, make_excerpt, query_terms};
use tantivy::doc;

#[test]
fn normalizes_case_and_deduplicates_query_terms() {
    assert_eq!(
        query_terms("Aquilum АКВИЛ aquilum"),
        vec!["aquilum".to_string(), "аквил".to_string()]
    );
}

#[test]
fn normalizes_diacritics_for_matching_and_highlighting() {
    assert_eq!(query_terms("BRÛLÉE"), vec!["brulee".to_string()]);
    let matches = find_prefix_matches("Crème brûlée", &["brulee".to_string()]);
    assert_eq!(&"Crème brûlée"[matches[0].0..matches[0].1], "brûlée");
}

#[test]
fn matches_only_word_prefixes_case_insensitively() {
    let matches = find_prefix_matches("Документ и псевдодокумент", &["док".to_string()]);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        &"Документ и псевдодокумент"[matches[0].0..matches[0].1],
        "Док"
    );
}

#[test]
fn excerpt_is_stable_and_keeps_the_first_match_visible() {
    let body = format!("{}needle {}", "word ".repeat(100), "tail ".repeat(100));
    let offset = body.find("needle").unwrap();
    let excerpt = make_excerpt(&body, Some(offset));
    assert!(excerpt.contains("needle"));
    assert!(excerpt.starts_with("..."));
    assert!(excerpt.ends_with("..."));
}

fn indexed(documents: &[(&str, &str, &str)]) -> (tempfile::TempDir, SearchIndex) {
    let directory = tempfile::tempdir().unwrap();
    let index = SearchIndex::open(directory.path()).unwrap();
    let mut writer = index.index.writer(15_000_000).unwrap();
    for (path, title, body) in documents {
        writer
            .add_document(doc!(
                index.fields.id => *path,
                index.fields.path => *path,
                index.fields.title => *title,
                index.fields.body => *body,
            ))
            .unwrap();
    }
    writer.commit().unwrap();
    index.reload().unwrap();
    (directory, index)
}

#[test]
fn disk_index_searches_title_and_body_by_case_insensitive_prefix() {
    let (_guard, index) = indexed(&[(
        "C:/vault/Квантовая заметка.md",
        "Квантовая заметка",
        "Внутри есть быстрый поиск и ещё один поиск.",
    )]);

    assert_eq!(index.search("КВАН", 10).unwrap().1.len(), 1);
    let body_results = index.search("ПОИ", 10).unwrap().1;
    assert_eq!(body_results[0].match_count, 2);
    assert!(body_results[0].snippet.contains("поиск"));
}

#[test]
fn a_multi_word_query_ranks_partial_matches_instead_of_returning_nothing() {
    let (_guard, index) = indexed(&[
        (
            "C:/vault/Камера.md",
            "Камера",
            "# Ощущение камеры\n\nФизика тика влияет на кадр.\n",
        ),
        (
            "C:/vault/Свет.md",
            "Свет",
            "# Освещение\n\nОдин источник на всю сцену.\n",
        ),
        (
            "C:/vault/Быстрота.md",
            "Быстрота",
            "# Плавность\n\nКадр рисуется быстро.\n",
        ),
    ]);

    let (_terms, results) = index.search("физика тика кадр плавность", 10).unwrap();
    assert_eq!(
        results.len(),
        2,
        "ни одна заметка не содержит все слова, но частичные совпадения возвращаются"
    );
    assert_eq!(
        results[0].path, "C:/vault/Камера.md",
        "заметка с большим числом совпавших слов идёт первой"
    );
    assert_eq!(results[0].matched_terms.len(), 3);
    assert_eq!(results[1].matched_terms.len(), 2);
    assert_eq!(
        results[0].heading.as_deref(),
        Some("Ощущение камеры"),
        "фрагмент подписан разделом, в котором нашлось совпадение"
    );
}
