pub fn strip_frontmatter(text: &str) -> String {
    let Some((author, body)) = parse(text) else {
        return text.to_owned();
    };
    match author.filter(|value| !value.is_empty()) {
        Some(name) => {
            if body.is_empty() {
                name
            } else {
                format!("{name}\n{body}")
            }
        }
        None => body,
    }
}

fn parse(text: &str) -> Option<(Option<String>, String)> {
    let (yaml, body) = block(text)?;
    Some((extract_author(yaml), body.to_owned()))
}

struct Bounds {
    yaml_start: usize,
    yaml_end: usize,
    body_start: usize,
}

fn block(text: &str) -> Option<(&str, &str)> {
    let found = bounds(text)?;
    let body = text[found.body_start..].trim_start_matches(['\r', '\n']);
    Some((&text[found.yaml_start..found.yaml_end], body))
}

fn bounds(text: &str) -> Option<Bounds> {
    let lead = text.len() - text.trim_start().len();
    let after_open = text[lead..].strip_prefix("---")?;
    let first_break = after_open.find('\n')?;
    if !after_open[..first_break].trim().is_empty() {
        return None;
    }
    let yaml_start = lead + "---".len() + first_break + 1;
    let mut cursor = yaml_start;
    while cursor <= text.len() {
        let rest = &text[cursor..];
        let (line, step) = match rest.find('\n') {
            Some(position) => (&rest[..position], position + 1),
            None => (rest, rest.len()),
        };
        if line.trim() == "---" {
            return Some(Bounds {
                yaml_start,
                yaml_end: cursor,
                body_start: cursor + step,
            });
        }
        if step == 0 {
            break;
        }
        cursor += step;
    }
    None
}

pub fn body(text: &str) -> &str {
    bounds(text).map_or(text, |found| &text[found.body_start..])
}

pub fn yaml_block(text: &str) -> Option<&str> {
    block(text).map(|(yaml, _)| yaml)
}

pub fn set(text: &str, updates: &[(String, String)]) -> Result<String, String> {
    if updates.is_empty() {
        return Ok(text.to_owned());
    }
    let Some(found) = bounds(text) else {
        if text.trim_start().starts_with("---") {
            return Err(
                "Блок frontmatter открыт, но не закрыт разделителем «---» — исправьте заметку вручную"
                    .to_owned(),
            );
        }
        let created = updates
            .iter()
            .map(|(key, value)| entry(key, value))
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(format!("---\n{created}\n---\n{text}"));
    };

    let mut lines = text[found.yaml_start..found.yaml_end]
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for (key, value) in updates {
        match lines.iter().position(|line| declares(line, key)) {
            Some(index) => {
                let existing = lines[index]
                    .split_once(':')
                    .map_or_else(|| key.clone(), |(name, _)| name.trim().to_owned());
                lines[index] = entry(&existing, value);
            }
            None => lines.push(entry(key, value)),
        }
    }

    let mut yaml = lines.join("\n");
    if !yaml.is_empty() {
        yaml.push('\n');
    }
    Ok(format!(
        "{}{yaml}{}",
        &text[..found.yaml_start],
        &text[found.yaml_end..]
    ))
}

fn entry(key: &str, value: &str) -> String {
    format!("{key}: {value}")
}

fn declares(line: &str, key: &str) -> bool {
    line.split_once(':')
        .is_some_and(|(name, _)| same_key(name, key))
}

fn same_key(left: &str, right: &str) -> bool {
    left.trim().to_lowercase() == right.trim().to_lowercase()
}

pub fn field(text: &str, wanted: &str) -> Option<String> {
    extract(block(text)?.0, wanted)
}

fn extract_author(yaml: &str) -> Option<String> {
    extract(yaml, "author")
}

fn extract(yaml: &str, wanted: &str) -> Option<String> {
    for line in yaml.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case(wanted) {
            continue;
        }
        let value = unquote(value.trim());
        if !value.is_empty() {
            return Some(value);
        }
    }
    None
}

pub fn unquote(value: &str) -> String {
    if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
        || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
    {
        value[1..value.len() - 1].trim().to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{set, strip_frontmatter};
    use crate::search::fields::note_fields;

    fn keys(text: &str) -> Vec<String> {
        note_fields(text)
            .into_iter()
            .map(|field| field.key)
            .collect()
    }

    fn value_of(text: &str, key: &str) -> String {
        note_fields(text)
            .into_iter()
            .find(|field| field.key == key)
            .map(|field| field.text)
            .unwrap_or_default()
    }

    const BOOK: &str = "---\ncreated: 31-08-2026 | (19:31)\ncover: true\ntype: book\nauthor: Иван Ефремов\nПуть к файлу: \n---\nТело заметки.\n";

    fn update(key: &str, value: &str) -> Vec<(String, String)> {
        vec![(key.to_owned(), value.to_owned())]
    }

    #[test]
    fn rewrites_a_field_in_place_and_leaves_the_body_alone() {
        let updated = set(BOOK, &update("author", "Рэй Брэдбери")).unwrap();
        assert!(updated.contains("author: Рэй Брэдбери"));
        assert!(!updated.contains("Иван Ефремов"));
        assert!(
            updated.ends_with("---\nТело заметки.\n"),
            "тело заметки не тронуто: {updated}"
        );
        assert_eq!(keys(&updated).len(), 5, "новых ключей не появилось");
    }

    #[test]
    fn appends_an_unknown_field_after_the_existing_ones() {
        let updated = set(BOOK, &update("rating", "5")).unwrap();
        let found = keys(&updated);
        assert_eq!(found.first().unwrap(), "created", "порядок прежних ключей сохранён");
        assert_eq!(found.last().unwrap(), "rating");
    }

    #[test]
    fn finds_the_key_regardless_of_case_and_keeps_its_original_spelling() {
        let updated = set(BOOK, &update("ПУТЬ К ФАЙЛУ", "Files/книга.epub")).unwrap();
        assert!(
            updated.contains("Путь к файлу: Files/книга.epub"),
            "написание ключа осталось прежним: {updated}"
        );
        assert_eq!(keys(&updated).len(), 5, "дубликат ключа не создан");
    }

    #[test]
    fn creates_the_block_for_a_note_that_had_none() {
        let updated = set("Только текст.\n", &update("type", "book")).unwrap();
        assert_eq!(updated, "---\ntype: book\n---\nТолько текст.\n");
    }

    #[test]
    fn refuses_to_touch_an_unclosed_block() {
        let broken = "---\ntype: book\nтело без закрывающего разделителя\n";
        assert!(
            set(broken, &update("type", "note")).is_err(),
            "второй блок поверх сломанного не создаётся"
        );
    }

    #[test]
    fn keeps_neighbouring_values_with_colons_untouched() {
        let note = "---\nsource: https://example.com/page\n---\nтекст";
        assert_eq!(value_of(note, "source"), "https://example.com/page");
        let updated = set(note, &update("author", "О. Генри")).unwrap();
        assert!(updated.contains("source: https://example.com/page"));
        assert!(updated.contains("author: О. Генри"));
    }

    const SAMPLE: &str = r#"---
type: book
author: Isaac Asimov
status: to-read
pages: 0/0
tags: []
rating: 0
cover_url: https://cdn.example.com/cover.jpg
page_cover_url: 
---
# Foundation

Actual note body."#;

    #[test]
    fn keeps_only_author_from_frontmatter() {
        let cleaned = strip_frontmatter(SAMPLE);
        assert!(cleaned.contains("Isaac Asimov"));
        assert!(cleaned.contains("Actual note body"));
        assert!(!cleaned.contains("type:"));
        assert!(!cleaned.contains("to-read"));
        assert!(!cleaned.contains("cover_url"));
        assert!(!cleaned.contains("tags:"));
    }

    #[test]
    fn notes_without_frontmatter_pass_through() {
        let text = "plain note without yaml";
        assert_eq!(strip_frontmatter(text), text);
    }

    #[test]
    fn empty_author_yields_body_only() {
        let text = "---\ntype: book\nauthor:\nstatus: x\n---\nbody";
        let cleaned = strip_frontmatter(text);
        assert_eq!(cleaned, "body");
        assert!(!cleaned.contains("type"));
    }
}
