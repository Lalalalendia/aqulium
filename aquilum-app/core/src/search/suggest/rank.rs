use super::store::Candidate;
use crate::search::models::NoteSuggestion;
use std::path::Path;

const WORD_BOUNDARIES: [char; 8] = [' ', '-', '_', '.', ',', '(', '[', '/'];

pub fn rank(mut candidates: Vec<Candidate>, key: &str, limit: usize) -> Vec<NoteSuggestion> {
    candidates.sort_by(|left, right| {
        tier(&left.title_key, key)
            .cmp(&tier(&right.title_key, key))
            .then_with(|| {
                left.title_key
                    .chars()
                    .count()
                    .cmp(&right.title_key.chars().count())
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    candidates
        .into_iter()
        .take(limit)
        .map(|candidate| suggestion(&candidate.path))
        .collect()
}

fn tier(title_key: &str, key: &str) -> u8 {
    if title_key.starts_with(key) {
        return 0;
    }
    if starts_word(title_key, key) {
        return 1;
    }
    2
}

fn starts_word(title_key: &str, key: &str) -> bool {
    title_key.match_indices(key).any(|(index, _)| {
        title_key[..index]
            .chars()
            .next_back()
            .is_some_and(|character| WORD_BOUNDARIES.contains(&character))
    })
}

fn suggestion(raw: &str) -> NoteSuggestion {
    NoteSuggestion {
        title: Path::new(raw)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        path: raw.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::store::Candidate;
    use super::rank;
    use std::path::Path;

    fn candidate(path: &str) -> Candidate {
        Candidate {
            path: path.to_owned(),
            title_key: Path::new(path)
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .to_lowercase(),
        }
    }

    #[test]
    fn name_start_beats_word_start_beats_middle() {
        let titles = rank(
            vec![
                candidate("C:/vault/Кто такой аудмитрий.md"),
                candidate("C:/vault/Заметки/Иван Дмитриев.md"),
                candidate("C:/vault/Дмитрий Чаплинский.md"),
            ],
            "дми",
            3,
        );
        assert_eq!(
            titles
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            [
                "Дмитрий Чаплинский",
                "Иван Дмитриев",
                "Кто такой аудмитрий"
            ],
        );
    }

    #[test]
    fn title_is_file_stem_and_path_is_kept() {
        let titles = rank(
            vec![candidate("C:/vault/Заметки/Иван Дмитриев.md")],
            "дми",
            1,
        );
        assert_eq!(titles[0].title, "Иван Дмитриев");
        assert_eq!(titles[0].path, "C:/vault/Заметки/Иван Дмитриев.md");
    }

    #[test]
    fn limit_cuts_the_tail() {
        let titles = rank(
            vec![
                candidate("C:/vault/Дмитрий Чаплинский.md"),
                candidate("C:/vault/Дмитрий Иванов.md"),
                candidate("C:/vault/Дмитрий Петров.md"),
            ],
            "дми",
            2,
        );
        assert_eq!(titles.len(), 2);
    }
}
