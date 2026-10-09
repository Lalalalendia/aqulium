use super::*;
use crate::documents::resolve::SyncPointLookup;

fn note(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("aquilum-legacy-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    let path = directory.join(format!("{name}.md"));
    std::fs::write(&path, "текст\n").expect("note");
    path
}

fn replica(path: &Path, point: Option<LegacyPoint>) -> LegacyReplica {
    LegacyReplica { path: path.to_string_lossy().into_owned(), updates: vec![b"update".to_vec()], point }
}

#[test]
fn moves_a_legacy_replica_with_its_sync_point_into_the_store() {
    let store = DocumentStore::memory().expect("store");
    let path = note("moved");
    let point = LegacyPoint { file_hash: "f".into(), text_hash: "t".into() };
    assert!(import_one(&store, &replica(&path, Some(point))));
    let key = identity(&path);
    assert_eq!(store.load(&key).expect("load"), vec![b"update".to_vec()]);
    assert_eq!(
        store.sync_point(&key),
        SyncPointLookup::Found(SyncPoint { file_hash: "f".into(), text_hash: "t".into() }),
    );
}

#[test]
fn never_overrides_a_replica_the_core_already_keeps() {
    let mut store = DocumentStore::memory().expect("store");
    let path = note("kept");
    store.compact(&identity(&path), b"current").expect("compact");
    assert!(!import_one(&store, &replica(&path, None)));
    assert_eq!(store.load(&identity(&path)).expect("load"), vec![b"current".to_vec()]);
}

#[test]
fn skips_a_note_that_no_longer_exists() {
    let store = DocumentStore::memory().expect("store");
    let path = note("gone");
    std::fs::remove_file(&path).expect("remove");
    assert!(!import_one(&store, &replica(&path, None)));
    assert!(!store.has_replica(&identity(&path)).expect("query"));
}
