use super::models::{SaveStateBatchInput, SessionStateInput};
use super::tests::{batch, setup};
use uuid::Uuid;

#[test]
fn active_only_batch_keeps_existing_tabs() {
    let (mut database, workspace_id, document_id, epoch) = setup();
    let initial = batch(workspace_id, document_id, epoch, 1, 20);
    let active_tab_id = initial.session.as_ref().unwrap().active_tab_id;
    database.save_batch(&initial).expect("initial");
    database
        .save_batch(&SaveStateBatchInput {
            workspace_id,
            window_id: "main".to_owned(),
            epoch,
            sequence: 2,
            now_ms: 20,
            session: Some(SessionStateInput {
                active_tab_id,
                tabs: None,
            }),
            views: Vec::new(),
            graph_camera: None,
        })
        .expect("active only");
    let loaded = database.load_session(workspace_id, "main").expect("load");
    assert_eq!(loaded.tabs.len(), 1);
}

#[test]
fn active_only_batch_rejects_unknown_tab() {
    let (mut database, workspace_id, document_id, epoch) = setup();
    database
        .save_batch(&batch(workspace_id, document_id, epoch, 1, 20))
        .expect("initial");
    let result = database.save_batch(&SaveStateBatchInput {
        workspace_id,
        window_id: "main".to_owned(),
        epoch,
        sequence: 2,
        now_ms: 20,
        session: Some(SessionStateInput {
            active_tab_id: Some(Uuid::new_v4()),
            tabs: None,
        }),
        views: Vec::new(),
        graph_camera: None,
    });
    assert!(result.is_err());
}
