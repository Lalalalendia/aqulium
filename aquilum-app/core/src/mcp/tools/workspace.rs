use super::super::bridge;
use super::super::vault::Vault;
use super::workspace_locator::{
    address, known_paths, known_workspace, open_guest, wait_until,
};
pub use super::workspace_locator::workspace_vault;
use super::{optional_text, text, tool, workspace_property};
use crate::search::paths::{canonical_path, same_path};
use serde_json::{json, Value};
use std::path::Path;
use std::time::Duration;
use crate::app_core::Core;

const SWITCH_TIMEOUT: Duration = Duration::from_secs(20);

pub fn definitions() -> Vec<Value> {
    vec![
        tool(
            "get_workspace",
            "Текущая база знаний: путь, название, открытая сейчас заметка (activeNote) и готовность индекса.",
            json!({}),
            &[],
        ),
        tool(
            "list_workspaces",
            "Все базы знаний, которые открывались в приложении. address — кратчайший однозначный хвост пути: его и передавайте другим инструментам аргументом workspace.",
            json!({}),
            &[],
        ),
        tool(
            "switch_workspace",
            "Переключить видимую базу знаний в окне интерфейса пользователя. ВНИМАНИЕ: это визуальное переключение окна приложения! Вызывать ТОЛЬКО если пользователь прямо попросил переключить базу на экране. Для чтения, поиска, создания и редактирования заметок в другой базе знаний НЕ вызывайте switch_workspace — передавайте аргумент workspace в нужный инструмент (read_note, update_note, search_notes и др.), они работают в фоне без переключения экрана пользователя.",
            json!({
                "path": { "type": "string", "description": "Путь к папке базы знаний или её address из list_workspaces" },
                "workspace": { "type": "string", "description": "Синоним path: путь к папке базы знаний или её address из list_workspaces" },
                "note": { "type": "string", "description": "Заметка, которую открыть после переключения" },
            }),
            &[],
        ),
        tool(
            "open_note",
            "Открыть заметку в интерфейсе приложения и сделать её активной вкладкой. ВНИМАНИЕ: это визуальное переключение экрана пользователя! Вызывать ТОЛЬКО по прямой просьбе пользователя открыть заметку на экране. Заметка из другой базы (workspace) переключит приложение на эту базу.",
            json!({
                "note": { "type": "string", "description": "Путь или название заметки" },
                "workspace": workspace_property(),
            }),
            &["note"],
        ),
    ]
}

pub fn call(
    core: &Core,
    name: &str,
    arguments: &Value,
) -> Option<Result<Value, String>> {
    Some(match name {
        "get_workspace" => get_workspace(core),
        "list_workspaces" => list_workspaces(core),
        "switch_workspace" => switch_workspace(core, arguments),
        "open_note" => open_note(core, arguments),
        _ => return None,
    })
}

pub fn requested_vault(core: &Core, arguments: &Value) -> Result<Vault, String> {
    match optional_text(arguments, "workspace") {
        Some(input) => workspace_vault(core, &input),
        None => Vault::active(core),
    }
}

pub fn indexed_vault(core: &Core, arguments: &Value) -> Result<Vault, String> {
    let vault = requested_vault(core, arguments)?;
    if !vault.visible {
        open_guest(core, &vault.root)?;
    }
    Ok(vault)
}

fn get_workspace(core: &Core) -> Result<Value, String> {
    let vault = Vault::active(core)?;
    let status = core.search.status(None);
    let active = core.active_note.get();
    Ok(json!({
        "path": vault.root.to_string_lossy(),
        "name": vault.root.file_name().map(|name| name.to_string_lossy().into_owned()),
        "activeNote": active.as_deref().map(|note| vault.relative(note)),
        "indexedDocuments": status.indexed_documents,
        "indexing": status.updating,
    }))
}

fn list_workspaces(core: &Core) -> Result<Value, String> {
    let active = core.search.active_root();
    let known = known_paths(core)?;
    let items = known
        .iter()
        .map(|root| {
            json!({
                "path": root.to_string_lossy(),
                "address": address(&known, root),
                "active": active.as_ref().is_some_and(|active| same_path(active, root)),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "workspaces": items }))
}

fn switch_workspace(core: &Core, arguments: &Value) -> Result<Value, String> {
    let input = optional_text(arguments, "path")
        .or_else(|| optional_text(arguments, "workspace"))
        .or_else(|| optional_text(arguments, "address"))
        .ok_or_else(|| "Не указана база знаний (параметр «path» или «workspace»)".to_string())?;
    let path = known_workspace(core, &input)?;
    switch_to(core, &path, optional_text(arguments, "note").as_deref())
}

fn open_note(core: &Core, arguments: &Value) -> Result<Value, String> {
    let vault = requested_vault(core, arguments)?;
    let note = text(arguments, "note")?;
    if !vault.visible {
        return switch_to(core, &vault.root, Some(&note));
    }
    let target = vault.note(&note)?;
    bridge::open_note(core, &target, true);
    Ok(json!({ "opened": vault.relative(&target) }))
}

fn switch_to(core: &Core, path: &Path, note: Option<&str>) -> Result<Value, String> {
    bridge::switch_workspace(core, path);
    let root = canonical_path(path);
    let service = &core.search;
    let switched = wait_until(SWITCH_TIMEOUT, || {
        service
            .active_root()
            .is_some_and(|active| same_path(&active, &root))
    });
    if !switched {
        return Err("Приложение не успело переключиться на эту базу знаний".to_owned());
    }

    let vault = Vault::active(core)?;
    let opened = match note {
        Some(note) => {
            let target = vault.note(note)?;
            bridge::open_note(core, &target, false);
            Some(vault.relative(&target))
        }
        None => None,
    };
    Ok(json!({
        "workspace": vault.root.to_string_lossy(),
        "opened": opened,
    }))
}
