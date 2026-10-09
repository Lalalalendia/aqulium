use super::*;
use std::cell::Cell;

const FILE: &str = "текст файла";
const DOC: &str = "текст документа";

struct Case<'a> {
    session_base: Option<&'a str>,
    file_hash: &'a str,
    doc_text: &'a str,
    lookup: SyncPointLookup,
    doc_hash: &'a str,
}

impl Default for Case<'_> {
    fn default() -> Self {
        Self {
            session_base: None,
            file_hash: "file-old",
            doc_text: DOC,
            lookup: SyncPointLookup::Found(SyncPoint {
                file_hash: "file-old".into(),
                text_hash: "text-old".into(),
            }),
            doc_hash: "text-old",
        }
    }
}

fn verdict(case: Case<'_>) -> SyncVerdict {
    let inputs = SyncInputs {
        session_base: case.session_base,
        file_hash: case.file_hash,
        file_text: FILE,
        doc_text: case.doc_text,
    };
    resolve_sync(inputs, || (case.lookup, case.doc_hash.to_owned()))
}

#[test]
fn does_nothing_when_file_and_document_already_agree() {
    assert_eq!(verdict(Case { doc_text: FILE, ..Case::default() }), SyncVerdict::InSync);
}

#[test]
fn merges_through_the_session_base_while_the_document_is_open() {
    assert_eq!(
        verdict(Case { session_base: Some("общая основа"), ..Case::default() }),
        SyncVerdict::Merge { base: "общая основа".into() },
    );
}

#[test]
fn takes_the_file_for_a_replica_that_was_never_filled() {
    assert_eq!(
        verdict(Case { lookup: SyncPointLookup::Absent, doc_text: "", ..Case::default() }),
        SyncVerdict::TakeFile,
    );
}

#[test]
fn does_not_read_the_sync_point_when_cheap_checks_decide() {
    let read = Cell::new(false);
    let inputs = SyncInputs { session_base: None, file_hash: "h", file_text: FILE, doc_text: FILE };
    let result = resolve_sync(inputs, || {
        read.set(true);
        (SyncPointLookup::Absent, String::new())
    });
    assert_eq!(result, SyncVerdict::InSync);
    assert!(!read.get());
}

#[test]
fn publishes_the_document_when_the_file_is_what_we_last_wrote() {
    assert_eq!(verdict(Case { doc_hash: "text-new", ..Case::default() }), SyncVerdict::PublishDocument);
}

#[test]
fn takes_the_file_when_only_the_file_moved_on() {
    assert_eq!(
        verdict(Case { file_hash: "file-new", ..Case::default() }),
        SyncVerdict::Merge { base: DOC.into() },
    );
}

#[test]
fn reports_a_conflict_when_both_sides_moved_from_a_known_base() {
    assert_eq!(
        verdict(Case { file_hash: "file-new", doc_hash: "text-new", ..Case::default() }),
        SyncVerdict::Conflict(ConflictCause::Diverged),
    );
}

#[test]
fn takes_the_file_as_an_external_edit_when_no_point_was_ever_written() {
    assert_eq!(
        verdict(Case { lookup: SyncPointLookup::Absent, ..Case::default() }),
        SyncVerdict::Merge { base: DOC.into() },
    );
}

#[test]
fn never_picks_a_side_silently_when_the_store_is_broken() {
    assert_eq!(
        verdict(Case { lookup: SyncPointLookup::Unavailable, ..Case::default() }),
        SyncVerdict::Conflict(ConflictCause::NoSyncPoint),
    );
}
