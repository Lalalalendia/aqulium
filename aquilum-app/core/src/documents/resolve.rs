use super::conflict::ConflictCause;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPoint {
    pub file_hash: String,
    pub text_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncPointLookup {
    Found(SyncPoint),
    Absent,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncVerdict {
    InSync,
    TakeFile,
    PublishDocument,
    Merge { base: String },
    Conflict(ConflictCause),
}

pub struct SyncInputs<'a> {
    pub session_base: Option<&'a str>,
    pub file_hash: &'a str,
    pub file_text: &'a str,
    pub doc_text: &'a str,
}

pub fn resolve_sync(
    inputs: SyncInputs<'_>,
    evidence: impl FnOnce() -> (SyncPointLookup, String),
) -> SyncVerdict {
    if inputs.file_text == inputs.doc_text {
        return SyncVerdict::InSync;
    }
    if let Some(base) = inputs.session_base {
        return SyncVerdict::Merge { base: base.to_owned() };
    }
    if inputs.doc_text.is_empty() {
        return SyncVerdict::TakeFile;
    }
    let (lookup, doc_hash) = evidence();
    let point = match lookup {
        SyncPointLookup::Absent => return SyncVerdict::Merge { base: inputs.doc_text.to_owned() },
        SyncPointLookup::Unavailable => return SyncVerdict::Conflict(ConflictCause::NoSyncPoint),
        SyncPointLookup::Found(point) => point,
    };
    let file_untouched = inputs.file_hash == point.file_hash;
    let document_untouched = doc_hash == point.text_hash;
    match (file_untouched, document_untouched) {
        (true, false) => SyncVerdict::PublishDocument,
        (false, true) => SyncVerdict::Merge { base: inputs.doc_text.to_owned() },
        _ => SyncVerdict::Conflict(ConflictCause::Diverged),
    }
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
