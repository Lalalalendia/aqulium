use crate::search::error::SearchError;
use crate::search::index::SearchIndex;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::TermQuery;
use tantivy::schema::{Field, IndexRecordOption, Value};
use tantivy::{Searcher, TantivyDocument, Term};

pub fn load_source(
    searcher: &Searcher,
    index: &SearchIndex,
    document_path: &Path,
) -> Result<(String, String), SearchError> {
    let absolute = document_path.to_string_lossy();
    let query = TermQuery::new(
        Term::from_field_text(index.fields.id, absolute.as_ref()),
        IndexRecordOption::Basic,
    );
    if let Some((_score, address)) = searcher
        .search(&query, &TopDocs::with_limit(1).order_by_score())?
        .into_iter()
        .next()
    {
        let document = searcher.doc::<TantivyDocument>(address)?;
        let body = stored_text(&document, index.fields.body).unwrap_or_default();
        let stored_title = stored_text(&document, index.fields.title)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| title(absolute.as_ref()));
        if !body.is_empty() || !stored_title.is_empty() {
            return Ok((stored_title, body));
        }
    }

    let body = crate::files::document::read_text(document_path)?;
    Ok((title(absolute.as_ref()), body))
}

pub fn stored_text(document: &TantivyDocument, field: Field) -> Option<String> {
    document
        .get_first(field)
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

pub fn title(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_owned()
}
