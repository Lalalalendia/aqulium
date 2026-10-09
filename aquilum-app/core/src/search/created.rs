use super::analyzer::frontmatter_field;
use super::note_date;
use std::cmp::Ordering;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CreatedSource {
    ModifiedTime = 0,
    FileBirthTime = 1,
    Frontmatter = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreatedAt {
    pub nanos: i64,
    pub source: CreatedSource,
}

impl CreatedSource {
    pub fn stored(self) -> i64 {
        self as i64
    }

    pub fn from_stored(value: i64) -> Self {
        match value {
            2 => Self::Frontmatter,
            1 => Self::FileBirthTime,
            _ => Self::ModifiedTime,
        }
    }
}

pub fn from_file_metadata(metadata: &fs::Metadata) -> CreatedAt {
    let modified = nanos(metadata.modified().ok()).unwrap_or_default();
    match nanos(metadata.created().ok()) {
        Some(birth) if birth <= modified || modified == 0 => CreatedAt {
            nanos: birth,
            source: CreatedSource::FileBirthTime,
        },
        _ => CreatedAt {
            nanos: modified,
            source: CreatedSource::ModifiedTime,
        },
    }
}

const FRONTMATTER_KEYS: [&str; 3] = ["created", "created_at", "date-created"];

pub fn declared(body: &str) -> Option<CreatedAt> {
    FRONTMATTER_KEYS
        .iter()
        .find_map(|key| frontmatter_field(body, key))
        .and_then(|value| note_date::parse(&value))
        .filter(|nanos| *nanos > 0)
        .map(|nanos| CreatedAt {
            nanos,
            source: CreatedSource::Frontmatter,
        })
}

pub fn resolve(stored: Option<CreatedAt>, candidate: CreatedAt) -> CreatedAt {
    let Some(stored) = stored.filter(|value| value.nanos > 0) else {
        return candidate;
    };
    if candidate.nanos == 0 {
        return stored;
    }
    match candidate.source.cmp(&stored.source) {
        Ordering::Greater => candidate,
        Ordering::Less => stored,
        Ordering::Equal if stored.nanos < candidate.nanos => stored,
        Ordering::Equal => candidate,
    }
}

fn nanos(time: Option<SystemTime>) -> Option<i64> {
    time.and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
        .filter(|value| *value > 0)
}

#[cfg(test)]
mod tests {
    use super::{declared, resolve, CreatedAt, CreatedSource};

    const NOTE: &str = "---
type: term
created: 07-10-2024
author: Jakob Nielsen
---
body";

    fn birth(nanos: i64) -> CreatedAt {
        CreatedAt {
            nanos,
            source: CreatedSource::FileBirthTime,
        }
    }

    fn modified(nanos: i64) -> CreatedAt {
        CreatedAt {
            nanos,
            source: CreatedSource::ModifiedTime,
        }
    }

    #[test]
    fn first_observation_is_taken_as_is() {
        assert_eq!(resolve(None, modified(500)), modified(500));
    }

    #[test]
    fn modification_time_moved_forward_does_not_move_the_date() {
        assert_eq!(resolve(Some(modified(500)), modified(900)), modified(500));
    }

    #[test]
    fn earlier_evidence_of_the_same_authority_wins() {
        assert_eq!(resolve(Some(modified(900)), modified(500)), modified(500));
    }

    #[test]
    fn stronger_source_replaces_a_weaker_one_even_when_later() {
        assert_eq!(resolve(Some(modified(500)), birth(900)), birth(900));
    }

    #[test]
    fn weaker_source_never_replaces_a_stronger_one() {
        assert_eq!(resolve(Some(birth(900)), modified(500)), birth(900));
    }

    #[test]
    fn unreadable_timestamps_keep_the_stored_date() {
        assert_eq!(resolve(Some(birth(900)), modified(0)), birth(900));
    }

    #[test]
    fn stored_zero_is_replaced_by_any_observation() {
        assert_eq!(resolve(Some(modified(0)), modified(700)), modified(700));
    }

    #[test]
    fn a_date_written_in_the_note_beats_every_file_date() {
        let from_note = declared(NOTE).expect("declared date");

        assert_eq!(from_note.source, CreatedSource::Frontmatter);
    }

    #[test]
    fn an_edited_declaration_replaces_the_stored_one_even_when_later() {
        let stored = declared(NOTE).expect("declared date");
        let edited = declared("---
created: 08-10-2024
---
body").expect("edited date");

        assert!(edited.nanos > stored.nanos);
        assert_eq!(edited.source, CreatedSource::Frontmatter);
    }

    #[test]
    fn what_the_migration_writes_is_what_the_indexer_reads() {
        let migrated = "---
created: 07-10-2024 17:17
---

Тип: #term
";
        let plain = declared("---
created: 07-10-2024
---
body").expect("date only");

        let with_time = declared(migrated).expect("date and time");

        assert_eq!(with_time.source, CreatedSource::Frontmatter);
        assert_eq!(with_time.nanos - plain.nanos, (17 * 3600 + 17 * 60) * 1_000_000_000);
    }

    #[test]
    fn a_note_without_a_declaration_keeps_using_its_file() {
        assert!(declared("---
type: term
---
body").is_none());
        assert!(declared("plain note").is_none());
    }

    #[test]
    fn an_unparsable_declaration_is_ignored_rather_than_guessed() {
        assert!(declared("---
created: вторая мировая
---
body").is_none());
        assert!(declared("---
created: 10/07/2024
---
body").is_none());
    }

    #[test]
    fn real_file_yields_a_date_not_later_than_its_modification() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("note.md");
        std::fs::write(&path, "body").expect("write note");
        let metadata = std::fs::metadata(&path).expect("metadata");

        let created = super::from_file_metadata(&metadata);
        let modified = super::nanos(metadata.modified().ok()).expect("modification time");

        assert!(created.nanos > 0);
        assert!(created.nanos <= modified);
    }
}
