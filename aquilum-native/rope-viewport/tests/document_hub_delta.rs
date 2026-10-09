//! End-to-end small edits through the *real* Aquilum DocumentHub/Yrs.
//! No full-document replace in the edit path and no production file changes.
use aquilum_native_common::NativeHost;
use aquilum_native_rope_viewport::TextModel;
use std::fs;

fn text(model: &TextModel) -> String {
    let mut out=Vec::new();
    model.write_to(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn incremental_unicode_edit_persists_through_real_document_hub() {
    let tmp=tempfile::tempdir().unwrap();
    let path=tmp.path().join("Книга-🌍.md");
    let initial="Привет 🌍\n# Заголовок\n";
    fs::write(&path,initial).unwrap();
    let host=NativeHost::new(&tmp.path().join("profile")).unwrap();
    let mut document=host.open(&path).unwrap();
    let mut model=TextModel::from_str(document.initial_text());

    let view=model.window_at_line(0,2,256).unwrap();
    let edit=model.propose_viewport_change(&view,7..9,"🌎").unwrap();
    let message=edit.to_changeset_json();
    assert_eq!(edit.utf16_range(),7..9);
    let expected_before=initial.encode_utf16().count();
    // CodeMirror JSON covers the original UTF-16 length with a compact edit.
    assert_eq!(message,serde_json::json!([7,[2,"🌎"],expected_before-9]));

    let prior=document.version();
    let acknowledged=document.push_json_change(message).unwrap();
    assert_eq!(acknowledged,prior+1);
    assert_eq!(model.commit_proposed_edit(edit),Ok(1));
    assert_eq!(text(&model),"Привет 🌎\n# Заголовок\n");

    drop(document);
    host.shutdown();
    assert_eq!(fs::read_to_string(&path).unwrap(),"Привет 🌎\n# Заголовок\n");
}

#[test]
fn stale_document_version_rejects_delta_without_mutating_secondary_rope() {
    let tmp=tempfile::tempdir().unwrap();
    let path=tmp.path().join("shared.md");
    fs::write(&path,"A🌍B\n").unwrap();
    let host=NativeHost::new(&tmp.path().join("profile")).unwrap();
    let mut first=host.open(&path).unwrap();
    let mut second=host.open(&path).unwrap();
    let mut model_one=TextModel::from_str(first.initial_text());
    let mut model_two=TextModel::from_str(second.initial_text());

    let one_view=model_one.window_at_line(0,1,128).unwrap();
    let one_edit=model_one.propose_viewport_change(&one_view,1..3,"🌎").unwrap();
    first.push_json_change(one_edit.to_changeset_json()).unwrap();
    model_one.commit_proposed_edit(one_edit).unwrap();

    let two_view=model_two.window_at_line(0,1,128).unwrap();
    let stale_edit=model_two.propose_viewport_change(&two_view,1..3,"🪐").unwrap();
    assert!(second.push_json_change(stale_edit.to_changeset_json()).is_err());
    assert_eq!(model_two.revision(),0);
    assert_eq!(text(&model_two),"A🌍B\n");
    drop(first);
    drop(second);
    host.shutdown();
    assert_eq!(fs::read_to_string(path).unwrap(),"A🌎B\n");
}

#[test]
fn edit_at_document_end_from_small_window_covers_earlier_utf16_units() {
    let tmp=tempfile::tempdir().unwrap();
    let path=tmp.path().join("long.md");
    let original="Ж🌍 строка\n".repeat(5000)+"A🌍B\n";
    fs::write(&path,&original).unwrap();
    let host=NativeHost::new(&tmp.path().join("profile")).unwrap();
    let mut document=host.open(&path).unwrap();
    let mut model=TextModel::from_str(document.initial_text());
    let visible=model.window_at_line(model.len_lines()-2,1,100).unwrap();
    assert_eq!(visible.text,"A🌍B\n");
    let edit=model.propose_viewport_change(&visible,1..3,"ok\n").unwrap();
    let json=edit.to_changeset_json();
    // ChangeSet has a *global* retain prefix even though viewport is local.
    assert!(json.as_array().unwrap()[0].as_u64().unwrap()>10_000);
    document.push_json_change(json).unwrap();
    model.commit_proposed_edit(edit).unwrap();
    drop(document);
    host.shutdown();
    assert_eq!(fs::read_to_string(path).unwrap(),"Ж🌍 строка\n".repeat(5000)+"Aok\nB\n");
}

#[test]
fn original_crlf_files_are_explicitly_normalized_by_existing_aquilum_core() {
    // This behavior is intentional in aquilum-app/core/src/files/document.rs:
    // read_file_snapshot_impl normalizes CRLF and lone CR to LF.
    // A native editor must mirror DocumentHub's opened text, not raw disk bytes.
    let tmp=tempfile::tempdir().unwrap();
    let path=tmp.path().join("windows-style.md");
    fs::write(&path,"Line 1\r\nПривет 🌍\r\n").unwrap();
    let host=NativeHost::new(&tmp.path().join("profile")).unwrap();
    let mut doc=host.open(&path).unwrap();
    assert_eq!(doc.initial_text(),"Line 1\nПривет 🌍\n");
    let mut model=TextModel::from_str(doc.initial_text());
    let view=model.window_at_line(1,1,120).unwrap();
    let plan=model.propose_viewport_change(&view,0..0,"X").unwrap();
    doc.push_json_change(plan.to_changeset_json()).unwrap();
    model.commit_proposed_edit(plan).unwrap();
    drop(doc);
    host.shutdown();
    assert_eq!(fs::read_to_string(&path).unwrap(),"Line 1\nXПривет 🌍\n");
}
