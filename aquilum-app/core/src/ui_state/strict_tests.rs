use super::database::UiStateDatabase;
use super::models::{OpenSessionInput, TabKind};
use uuid::Uuid;

fn session() -> (UiStateDatabase, Uuid) {
    let mut database = UiStateDatabase::memory().expect("database");
    let workspace_id = database
        .resolve_workspace("C:/notes", 1)
        .expect("workspace");
    database
        .open_session(&OpenSessionInput {
            workspace_id,
            window_id: "main".to_owned(),
            epoch: Uuid::new_v4(),
            now_ms: 1,
        })
        .expect("session");
    (database, workspace_id)
}

#[test]
fn a_graph_tab_survives_a_session_reload() {
    let (database, workspace_id) = session();
    let tab_id = Uuid::new_v4();
    database
        .connection
        .execute(
            "INSERT INTO tabs(workspace_id, window_id, tab_id, kind, position)
             VALUES(?1, 'main', ?2, 'graph', 0)",
            [workspace_id.to_string(), tab_id.to_string()],
        )
        .expect("tab");

    let loaded = database
        .load_session(workspace_id, "main")
        .expect("session reload");

    assert_eq!(loaded.tabs.len(), 1);
    assert_eq!(loaded.tabs[0].kind, TabKind::Graph);
    assert_eq!(loaded.tabs[0].document_id, None);
}

#[test]
fn unknown_tab_kind_is_not_silently_restored_as_empty() {
    let (database, workspace_id) = session();
    database
        .connection
        .execute(
            "INSERT INTO tabs(workspace_id, window_id, tab_id, kind, position)
             VALUES(?1, 'main', ?2, 'corrupt', 0)",
            [workspace_id.to_string(), Uuid::new_v4().to_string()],
        )
        .expect("tab");
    assert!(database.load_session(workspace_id, "main").is_err());
}
