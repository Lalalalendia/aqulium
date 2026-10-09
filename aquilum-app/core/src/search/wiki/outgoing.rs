use super::super::error::SearchError;
use super::super::models::OutgoingLink;
use super::parser::extract;
use super::resolver::resolve_many;
use super::LINK_LIST_LIMIT;
use rusqlite::Connection;
use std::path::Path;

pub fn outgoing_links(
    connection: &Connection,
    root: &Path,
    document: &Path,
) -> Result<Vec<OutgoingLink>, SearchError> {
    let body = crate::files::document::read_text(document)?;
    let targets = extract(&body)
        .into_iter()
        .take(LINK_LIST_LIMIT)
        .map(|link| link.target)
        .collect::<Vec<_>>();
    let paths = resolve_many(connection, root, document, &targets)?;
    let items = targets
        .into_iter()
        .zip(paths)
        .map(|(target, path)| OutgoingLink {
            title: path
                .as_deref()
                .and_then(Path::file_stem)
                .and_then(|value| value.to_str())
                .map(str::to_owned)
                .unwrap_or_else(|| target_title(&target)),
            path: path.map(|value| value.to_string_lossy().into_owned()),
            target,
        })
        .collect();

    Ok(items)
}

fn target_title(target: &str) -> String {
    let filename = target.trim().rsplit(['/', '\\']).next().unwrap_or_default();
    filename
        .get(..filename.len().saturating_sub(3))
        .filter(|_| {
            filename
                .get(filename.len().saturating_sub(3)..)
                .is_some_and(|ending| ending.eq_ignore_ascii_case(".md"))
        })
        .unwrap_or(filename)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::outgoing_links;
    use crate::search::wiki::{index_document, open_schema};
    use rusqlite::Connection;
    use std::fs;

    #[test]
    fn returns_resolved_and_missing_document_titles() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let source = root.join("Source.md");
        let target = root.join("Folder").join("Target.md");
        fs::create_dir(target.parent().unwrap()).unwrap();
        fs::write(
            &source,
            "[[Folder/Target|alias]] and [[Missing Note]] and [[Research.v2]]",
        )
        .unwrap();
        fs::write(&target, "target").unwrap();

        let mut connection = Connection::open_in_memory().unwrap();
        open_schema(&connection).unwrap();
        let transaction = connection.transaction().unwrap();
        index_document(
            &transaction,
            root,
            &source,
            "[[Folder/Target|alias]] and [[Missing Note]] and [[Research.v2]]",
        )
        .unwrap();
        index_document(&transaction, root, &target, "target").unwrap();
        transaction.commit().unwrap();

        let links = outgoing_links(&connection, root, &source).unwrap();
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].title, "Target");
        assert_eq!(links[0].path.as_deref(), target.to_str());
        assert_eq!(links[1].title, "Missing Note");
        assert!(links[1].path.is_none());
        assert_eq!(links[2].title, "Research.v2");
    }
}
