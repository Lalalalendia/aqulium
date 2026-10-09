use super::error::SearchError;
use super::index::SearchIndex;
use super::matching::{analyzed_query_terms, result_from_document};
use super::models::SearchResult;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, Query, TermQuery};
use tantivy::schema::{Field, IndexRecordOption};
use tantivy::{TantivyDocument, Term};

const MAX_RESULTS: usize = 100;
const MIN_PREFIX_CHARS: usize = 2;
const MIN_FUZZY_CHARS: usize = 5;

pub fn search(
    search_index: &SearchIndex,
    input: &str,
    limit: usize,
) -> Result<(Vec<String>, Vec<SearchResult>), SearchError> {
    let terms = analyzed_query_terms(input);
    if terms.is_empty() || limit == 0 {
        return Ok((terms, Vec::new()));
    }

    let query = combined_query(search_index, &terms);
    let searcher = search_index.reader.searcher();
    let top_docs = searcher.search(
        &query,
        &TopDocs::with_limit(limit.min(MAX_RESULTS)).order_by_score(),
    )?;
    let mut results = Vec::with_capacity(top_docs.len());
    for (_, address) in top_docs {
        let document = searcher.doc::<TantivyDocument>(address)?;
        if let Some(result) = result_from_document(&document, search_index.fields, &terms) {
            results.push(result);
        }
    }
    Ok((terms, results))
}

fn searchable_fields(index: &SearchIndex) -> [Field; 3] {
    [index.fields.title, index.fields.path, index.fields.body]
}

fn combined_query(index: &SearchIndex, terms: &[String]) -> BooleanQuery {
    let mut term_clauses = Vec::<(Occur, Box<dyn Query>)>::with_capacity(terms.len());
    let mut exact_clauses = Vec::<(Occur, Box<dyn Query>)>::new();

    for text in terms {
        let mut field_matches = Vec::<(Occur, Box<dyn Query>)>::with_capacity(3);
        for field in searchable_fields(index) {
            let term = Term::from_field_text(field, text);
            let boost = field_boost(index, field);
            field_matches.push((
                Occur::Should,
                Box::new(BoostQuery::new(field_matcher(&term, text), boost)),
            ));
            exact_clauses.push((
                Occur::Should,
                Box::new(BoostQuery::new(
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs)),
                    boost * 2.0,
                )),
            ));
        }
        term_clauses.push((Occur::Should, Box::new(BooleanQuery::new(field_matches))));
    }

    let any_term = BooleanQuery::with_minimum_required_clauses(term_clauses, 1);
    BooleanQuery::new(vec![
        (Occur::Must, Box::new(any_term)),
        (Occur::Should, Box::new(BooleanQuery::new(exact_clauses))),
    ])
}

fn field_matcher(term: &Term, text: &str) -> Box<dyn Query> {
    let length = text.chars().count();
    if length < MIN_PREFIX_CHARS {
        return Box::new(TermQuery::new(term.clone(), IndexRecordOption::WithFreqs));
    }
    let distance = u8::from(length >= MIN_FUZZY_CHARS);
    Box::new(FuzzyTermQuery::new_prefix(term.clone(), distance, true))
}

fn field_boost(index: &SearchIndex, field: Field) -> f32 {
    if field == index.fields.title {
        5.0
    } else if field == index.fields.path {
        2.0
    } else {
        1.0
    }
}
