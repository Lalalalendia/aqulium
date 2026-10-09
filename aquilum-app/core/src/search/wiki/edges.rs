use super::target::pick_candidate;
use crate::search::error::SearchError;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

pub struct WikiDocument {
    pub path: String,
    pub relative_key: String,
    pub title_key: String,
}

pub fn read_documents(connection: &Connection) -> Result<Vec<WikiDocument>, SearchError> {
    let mut statement =
        connection.prepare("SELECT path, relative_key, title_key FROM wiki_documents")?;
    let rows = statement.query_map([], |row| {
        Ok(WikiDocument {
            path: row.get(0)?,
            relative_key: row.get(1)?,
            title_key: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn for_each_link<'a, D>(
    connection: &Connection,
    root: &Path,
    documents: D,
    mut visit: impl FnMut(&str, &str),
) -> Result<(), SearchError>
where
    D: IntoIterator<Item = (&'a str, &'a str, &'a str)>,
{
    let mut by_title = HashMap::<&str, Vec<&str>>::new();
    let mut by_relative = HashMap::<&str, Vec<&str>>::new();
    for (path, relative_key, title_key) in documents {
        by_relative.entry(relative_key).or_default().push(path);
        by_title.entry(title_key).or_default().push(path);
    }

    let mut statement =
        connection.prepare("SELECT source_path, target_key, target_kind FROM wiki_links")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (source, key, kind) = row?;
        let candidates = if kind == "path" {
            by_relative.get(key.as_str())
        } else {
            by_title.get(key.as_str())
        };
        let Some(target) =
            candidates.and_then(|paths| pick_candidate(root, Path::new(&source), paths))
        else {
            continue;
        };
        visit(&source, target);
    }
    Ok(())
}
