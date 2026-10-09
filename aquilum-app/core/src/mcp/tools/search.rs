use super::workspace::{indexed_vault, requested_vault};
use super::{limit, optional_text, shorten, text, tool, workspace_property};
use crate::files::document::read_text;
use crate::search::analysis::models::AnalysisMethod;
use crate::search::headings;
use crate::wikixiv::models::SearchRequest;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::app_core::Core;

const SNIPPET_LIMIT: usize = 300;
const CONTEXT_LIMIT: usize = 200;
static WIKI_GENERATION: AtomicU64 = AtomicU64::new(1);

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "search_notes",
            "Полнотекстовый поиск по базе знаний со стеммингом. Слова запроса объединяются по «ИЛИ» и ранжируются, поэтому длинный запрос находит и частичные совпадения: смотрите matchedTerms, какие слова нашлись. У каждого результата есть heading — раздел с совпадением, и backlinks — сколько заметок на него ссылаются. Поле hasMore говорит, осталось ли что-то за пределами выдачи. Ищет по тексту заметок: frontmatter в полнотекстовый индекс не входит, по его полям ищет find_notes.",
            json!({
                "query": { "type": "string", "description": "Поисковый запрос: чем больше слов, тем выше в выдаче заметки, где нашлись все" },
                "folder": { "type": "string", "description": "Искать только в этой папке, пусто — вся база" },
                "limit": { "type": "integer", "description": "Сколько заметок вернуть, по умолчанию 10" },
                "workspace": workspace_property(),
            }),
            &["query"],
        ),
        tool(
            "dataview_syntax",
            "Язык блоков ```dataview: формы запроса, части, поля и функции — с примерами. Вызовите без аргументов, чтобы получить справку, или передайте query, чтобы проверить запрос перед вставкой в заметку.",
            json!({
                "query": { "type": "string", "description": "Текст запроса для проверки; пусто — вернуть справку" },
            }),
            &[],
        ),
        tool(
            "related_notes",
            "Похожие заметки по тексту (BM25F): что писать рядом, что дублируется, что развить.",
            json!({
                "note": { "type": "string", "description": "Путь или название заметки" },
                "limit": { "type": "integer", "description": "Сколько заметок вернуть, по умолчанию 10" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "linked_notes",
            "Связанные заметки по графу ссылок [[...]] (Adamic-Adar): общие соседи, а не общий текст.",
            json!({
                "note": { "type": "string", "description": "Путь или название заметки" },
                "limit": { "type": "integer", "description": "Сколько заметок вернуть, по умолчанию 10" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "backlinks",
            "Заметки, которые ссылаются на указанную: по одной строке на заметку, mentions — сколько раз ссылка встречается, heading и context — раздел и строка первого упоминания.",
            json!({
                "note": { "type": "string", "description": "Путь или название заметки" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "outgoing_links",
            "Ссылки [[...]] из указанной заметки, включая ещё не созданные, по одной строке на цель; mentions — сколько раз она упомянута.",
            json!({
                "note": { "type": "string", "description": "Путь или название заметки" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "wiki_sources",
            "Статьи Wikipedia по теме заметки.",
            json!({
                "note": { "type": "string", "description": "Путь или название заметки" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
        tool(
            "get_index_status",
            "Состояние поискового индекса: готов ли он и сколько заметок проиндексировано.",
            json!({
                "workspace": workspace_property(),
            }),
            &[],
        ),
    ]
}

pub fn call(
    core: &Core,
    name: &str,
    arguments: &Value,
) -> Option<Result<Value, String>> {
    Some(match name {
        "search_notes" => search_notes(core, arguments),
        "dataview_syntax" => dataview_syntax(arguments),
        "related_notes" => analyze(core, arguments, AnalysisMethod::Bm25f),
        "linked_notes" => analyze(core, arguments, AnalysisMethod::AdamicAdar),
        "backlinks" => backlinks(core, arguments),
        "outgoing_links" => outgoing_links(core, arguments),
        "wiki_sources" => wiki_sources(core, arguments),
        "get_index_status" => index_status(core, arguments),
        _ => return None,
    })
}

fn search_notes(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = indexed_vault(core, arguments)?;
    let folder = optional_text(arguments, "folder")
        .map(|value| vault.folder(&value))
        .transpose()?;
    if let Some(folder) = folder.as_ref().filter(|folder| !folder.is_dir()) {
        return Err(format!("Папка не найдена: {}", vault.relative(folder)));
    }
    let size = limit(arguments, 10, 50);
    let asked = if folder.is_some() {
        size.saturating_mul(4).min(200)
    } else {
        size + 1
    };
    let response = core
        .search
        .search(
            &vault.root.to_string_lossy(),
            &text(arguments, "query")?,
            Some(asked),
        )
        .map_err(|error| error.to_string())?;

    let mut inside = response.results.iter().filter(|result| {
        folder
            .as_ref()
            .is_none_or(|folder| Path::new(&result.path).starts_with(folder))
    });
    let shown = inside.by_ref().take(size).collect::<Vec<_>>();
    let counts = core
        .search
        .backlink_counts(
            &vault.root.to_string_lossy(),
            &shown.iter().map(|result| result.path.clone()).collect::<Vec<_>>(),
        )
        .ok();
    let results = shown
        .iter()
        .enumerate()
        .map(|(position, result)| {
            json!({
                "path": vault.relative(Path::new(&result.path)),
                "heading": result.heading,
                "snippet": shorten(&result.snippet, SNIPPET_LIMIT),
                "matches": result.match_count,
                "matchedTerms": result.matched_terms,
                "backlinks": counts.as_ref().and_then(|counts| counts.get(position)),
            })
        })
        .collect::<Vec<_>>();

    let has_more = inside.next().is_some();
    Ok(json!({
        "indexing": response.status.updating,
        "terms": response.query_terms,
        "results": results,
        "hasMore": has_more,
    }))
}

fn dataview_syntax(arguments: &Value) -> Result<Value, String> {
    let Some(query) = optional_text(arguments, "query") else {
        return Ok(json!({ "syntax": crate::search::dataview::syntax_help() }));
    };
    match crate::search::dataview::check_query(&query) {
        Ok(()) => Ok(json!({ "valid": true, "query": query })),
        Err(message) => Ok(json!({ "valid": false, "error": message })),
    }
}

fn analyze(core: &Core, arguments: &Value, method: AnalysisMethod) -> Result<Value, String> {
    let vault = indexed_vault(core, arguments)?;
    let note = vault.note(&text(arguments, "note")?)?;
    let config = core.settings.get_config();
    let results = core
        .search
        .analyze_document(
            &vault.root.to_string_lossy(),
            &note.to_string_lossy(),
            method,
            Some(limit(arguments, 10, 50)),
            &config,
        )
        .map_err(|error| error.to_string())?;

    let items = results
        .iter()
        .map(|result| {
            json!({
                "path": vault.relative(Path::new(&result.path)),
                "score": (result.raw_score * 100.0).round() / 100.0,
                "reasons": result.reasons,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "note": vault.relative(&note), "results": items }))
}

fn backlinks(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = indexed_vault(core, arguments)?;
    let note = vault.note(&text(arguments, "note")?)?;
    let links = core
        .search
        .backlinks(&vault.root.to_string_lossy(), &note.to_string_lossy())
        .map_err(|error| error.to_string())?;
    let mut sources = Vec::<(&str, Vec<usize>)>::new();
    for link in &links {
        match sources.last_mut() {
            Some((path, offsets)) if *path == link.path => offsets.push(link.offset),
            _ => sources.push((&link.path, vec![link.offset])),
        }
    }
    let items = sources
        .iter()
        .map(|(path, offsets)| {
            let (heading, context) = link_context(Path::new(path), offsets[0]);
            json!({
                "path": vault.relative(Path::new(path)),
                "mentions": offsets.len(),
                "heading": heading,
                "context": context,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "note": vault.relative(&note), "backlinks": items }))
}

fn outgoing_links(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = indexed_vault(core, arguments)?;
    let note = vault.note(&text(arguments, "note")?)?;
    let links = core
        .search
        .outgoing_links(&vault.root.to_string_lossy(), &note.to_string_lossy())
        .map_err(|error| error.to_string())?;
    let mut targets = Vec::<(usize, usize)>::new();
    for (position, link) in links.iter().enumerate() {
        match targets
            .iter_mut()
            .find(|(first, _)| links[*first].target == link.target)
        {
            Some((_, mentions)) => *mentions += 1,
            None => targets.push((position, 1)),
        }
    }
    let items = targets
        .iter()
        .map(|(first, mentions)| {
            let link = &links[*first];
            json!({
                "target": link.target,
                "path": link.path.as_deref().map(|path| vault.relative(Path::new(path))),
                "exists": link.path.is_some(),
                "mentions": mentions,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "note": vault.relative(&note), "links": items }))
}

fn wiki_sources(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = requested_vault(core, arguments)?;
    let note = vault.note(&text(arguments, "note")?)?;
    let content = read_text(&note).map_err(|error| error.to_string())?;
    let enabled = core
        .settings
        .get_config()
        .analysis
        .enable_wikixiv;
    let result = core
        .wikixiv
        .search(
            SearchRequest {
                text: content,
                document_path: Some(note.to_string_lossy().into_owned()),
                generation: WIKI_GENERATION.fetch_add(1, Ordering::Relaxed),
            },
            enabled,
        )?;

    let hits = result
        .hits
        .iter()
        .map(|hit| {
            json!({
                "title": hit.title,
                "url": hit.url,
                "snippet": shorten(&hit.snippet, SNIPPET_LIMIT),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "note": vault.relative(&note),
        "offline": result.offline,
        "insufficientText": result.insufficient_text,
        "sources": hits,
    }))
}

fn link_context(path: &Path, offset_utf16: usize) -> (Option<String>, Option<String>) {
    let Ok(text) = read_text(path) else {
        return (None, None);
    };
    let at = byte_at_utf16(&text, offset_utf16);
    let start = text[..at].rfind('\n').map_or(0, |index| index + 1);
    let end = text[at..].find('\n').map_or(text.len(), |index| at + index);
    let heading = headings::enclosing(&headings::headings(&text), at).map(|heading| heading.title.clone());
    (heading, Some(shorten(text[start..end].trim(), CONTEXT_LIMIT)))
}

fn byte_at_utf16(text: &str, offset: usize) -> usize {
    let mut units = 0;
    for (byte, symbol) in text.char_indices() {
        if units >= offset {
            return byte;
        }
        units += symbol.len_utf16();
    }
    text.len()
}

fn index_status(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = requested_vault(core, arguments)?;
    let status = core
        .search
        .status(Some(&vault.root.to_string_lossy()));
    serde_json::to_value(status).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{byte_at_utf16, link_context};
    use std::fs;

    #[test]
    fn a_utf16_offset_lands_on_the_same_character() {
        let text = "аб😀в";
        assert_eq!(&text[byte_at_utf16(text, 4)..], "в");
        assert_eq!(byte_at_utf16(text, 99), text.len());
    }

    #[test]
    fn the_context_is_the_line_and_section_of_the_link() {
        let directory = tempfile::tempdir().unwrap();
        let note = directory.path().join("Источник.md");
        fs::write(&note, "# Раздел

текст [[Цель]] хвост
дальше
").unwrap();
        let (heading, context) = link_context(&note, 16);
        assert_eq!(heading.as_deref(), Some("Раздел"));
        assert_eq!(context.as_deref(), Some("текст [[Цель]] хвост"));
    }
}
