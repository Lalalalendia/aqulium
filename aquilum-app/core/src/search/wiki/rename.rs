use super::super::error::SearchError;
use super::parser::extract;
use super::resolver::LinkResolver;
use super::target::{relative_key, relative_target, select_candidate, target_key, title_key};
use crate::files::document::{hash_bytes, normalize_line_endings};
use crate::search::paths::same_path;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

pub struct LinkRewrite {
    pub path: PathBuf,
    pub original: String,
    pub original_hash: String,
    pub content: String,
}

pub fn plan_for_rename(
    connection: &Connection,
    root: &Path,
    old_path: &Path,
    new_path: &Path,
    sources: Vec<PathBuf>,
    fallback_candidates: Option<&[String]>,
) -> Result<Vec<LinkRewrite>, SearchError> {
    let mut rewrites = Vec::new();
    let mut resolver = LinkResolver::new(connection)?;
    for stored_path in sources {
        let source_path = if same_path(&stored_path, old_path) {
            new_path
        } else {
            &stored_path
        };
        let Ok(original) = fs::read_to_string(source_path) else {
            continue;
        };
        let text = normalize_line_endings(original.clone());
        let mut changes = Vec::new();
        for link in extract(&text) {
            if targets_path(
                &mut resolver,
                root,
                &stored_path,
                &link.target,
                old_path,
                fallback_candidates,
            )? {
                let path_style = link.target.replace('\\', "/").contains('/');
                changes.push((
                    link.target_range,
                    relative_target(root, new_path, path_style),
                ));
            }
        }
        if changes.is_empty() {
            continue;
        }
        let mut content = text;
        for (range, replacement) in changes.into_iter().rev() {
            content.replace_range(range, &replacement);
        }
        let original_hash = hash_bytes(original.as_bytes());
        rewrites.push(LinkRewrite {
            path: stored_path,
            original,
            original_hash,
            content,
        });
    }
    Ok(rewrites)
}

fn targets_path(
    resolver: &mut LinkResolver<'_>,
    root: &Path,
    source: &Path,
    target: &str,
    old_path: &Path,
    fallback_candidates: Option<&[String]>,
) -> Result<bool, SearchError> {
    let (key, kind) = target_key(target);
    if let Some(candidates) = fallback_candidates {
        return Ok(if kind == "path" {
            key == relative_key(root, old_path)
        } else {
            key == title_key(old_path)
                && select_candidate(root, source, candidates)
                    .is_some_and(|path| same_path(&path, old_path))
        });
    }
    if let Some(path) = resolver.resolve(root, source, target)? {
        return Ok(same_path(&path, old_path));
    }
    Ok(if kind == "path" {
        key == relative_key(root, old_path)
    } else {
        key == title_key(old_path)
    })
}

#[cfg(test)]
mod tests {
    use super::plan_for_rename;
    use crate::search::wiki::{index_document, open_schema};
    use rusqlite::Connection;
    use std::fs;

    #[test]
    fn preserves_aliases_when_a_target_is_renamed() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let old_path = root.join("Old.md");
        let new_path = root.join("New.md");
        let source_path = root.join("Source.md");
        fs::write(&old_path, "target").unwrap();
        fs::write(&source_path, "[[Old]] and [[Old|visible]]").unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        open_schema(&connection).unwrap();
        let transaction = connection.transaction().unwrap();
        index_document(&transaction, root, &old_path, "target").unwrap();
        index_document(
            &transaction,
            root,
            &source_path,
            "[[Old]] and [[Old|visible]]",
        )
        .unwrap();
        transaction.commit().unwrap();

        let plan = plan_for_rename(
            &connection,
            root,
            &old_path,
            &new_path,
            vec![source_path],
            None,
        )
        .unwrap();

        assert_eq!(plan[0].content, "[[New]] and [[New|visible]]");
    }

    #[test]
    fn rewrites_links_in_a_crlf_note_and_keeps_the_hash_of_its_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let old_path = root.join("Old.md");
        let new_path = root.join("New.md");
        let source_path = root.join("Source.md");
        let raw = "# Title

see [[Old]]
";
        fs::write(&old_path, "target").unwrap();
        fs::write(&source_path, raw).unwrap();
        let mut connection = Connection::open_in_memory().unwrap();
        open_schema(&connection).unwrap();
        let transaction = connection.transaction().unwrap();
        index_document(&transaction, root, &old_path, "target").unwrap();
        index_document(&transaction, root, &source_path, "# Title

see [[Old]]
").unwrap();
        transaction.commit().unwrap();

        let plan = plan_for_rename(&connection, root, &old_path, &new_path, vec![source_path], None)
            .unwrap();

        assert_eq!(plan[0].content, "# Title

see [[New]]
");
        assert_eq!(plan[0].original, raw);
        assert_eq!(plan[0].original_hash, blake3::hash(raw.as_bytes()).to_hex().to_string());
    }
}
