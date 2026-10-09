pub mod score;
pub mod source;
pub mod terms;

#[cfg(test)]
mod tests;

use self::score::{Candidate, SegmentTerms};
use self::source::{load_source, stored_text, title};
use self::terms::{candidate_query, select_terms};
use super::models::AnalysisResult;
use crate::search::paths::identity;
use crate::search::error::SearchError;
use crate::search::index::SearchIndex;
use crate::settings::models::{Bm25fParams, SearchIndexSettings};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::{DocAddress, DocId, TantivyDocument};

pub fn analyze(
    index: &SearchIndex,
    document_path: &Path,
    limit: usize,
    params: &Bm25fParams,
    search: &SearchIndexSettings,
) -> Result<Vec<AnalysisResult>, SearchError> {
    let searcher = index.reader.searcher();
    let (source_title, source_body) = load_source(&searcher, index, document_path)?;
    let terms = select_terms(
        &searcher,
        index,
        &source_body,
        &source_title,
        search.max_query_terms,
    )?;
    if terms.is_empty() {
        return Ok(Vec::new());
    }

    let candidate_limit = search
        .candidate_pool_size
        .clamp(16, 2048)
        .min(searcher.num_docs() as usize);
    if candidate_limit == 0 {
        return Ok(Vec::new());
    }
    let hits = searcher.search(
        &candidate_query(index, &terms, params),
        &TopDocs::with_limit(candidate_limit).order_by_score(),
    )?;
    let field_stats = index.field_stats()?;
    let source_identity = identity(document_path);

    let mut by_segment: BTreeMap<u32, Vec<DocId>> = BTreeMap::new();
    for (_retrieval_score, address) in hits {
        by_segment
            .entry(address.segment_ord)
            .or_default()
            .push(address.doc_id);
    }

    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for (segment_ord, mut doc_ids) in by_segment {
        doc_ids.sort_unstable();
        doc_ids.dedup();
        let reader = searcher.segment_reader(segment_ord);
        let title_norms = reader.get_fieldnorms_reader(index.fields.title)?;
        let body_norms = reader.get_fieldnorms_reader(index.fields.body)?;
        let mut postings = SegmentTerms::open(reader, index.fields.title, index.fields.body, &terms)?;

        for doc_id in doc_ids {
            let address = DocAddress { segment_ord, doc_id };
            let candidate = searcher.doc::<TantivyDocument>(address)?;
            let path = stored_text(&candidate, index.fields.id).unwrap_or_default();
            if path.is_empty() {
                continue;
            }
            let candidate_identity = identity(Path::new(&path));
            if candidate_identity == source_identity || !seen.insert(candidate_identity) {
                continue;
            }

            let candidate_title = stored_text(&candidate, index.fields.title)
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| title(&path));
            let lengths = Candidate {
                id: doc_id,
                title_length: title_norms.fieldnorm(doc_id),
                body_length: body_norms.fieldnorm(doc_id),
            };
            let (raw_score, reasons) = postings.score(&terms, &lengths, &field_stats, params);
            if raw_score <= 0.0 || !raw_score.is_finite() {
                continue;
            }
            results.push(AnalysisResult {
                path,
                title: candidate_title,
                raw_score,
                similarity: None,
                confidence: None,
                reasons,
            });
        }
    }

    results.sort_by(|left, right| {
        right
            .raw_score
            .partial_cmp(&left.raw_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.title.cmp(&right.title))
    });
    results.truncate(limit);
    Ok(results)
}
