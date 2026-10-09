use super::merge::TextEdit;
use std::fmt;
use yrs::updates::decoder::Decode;
use yrs::branch::{Branch, BranchPtr};
use yrs::updates::encoder::Encode;
use yrs::{
    Assoc, Doc, GetString, IndexedSequence, OffsetKind, Options, ReadTxn, StateVector, StickyIndex, Text, TextRef,
    Transact, Update,
};

const TEXT_NAME: &str = "codemirror";

#[derive(Debug)]
pub struct ReplicaError(String);

impl fmt::Display for ReplicaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn failure(error: impl fmt::Display) -> ReplicaError {
    ReplicaError(error.to_string())
}

pub struct Replica {
    doc: Doc,
    text: TextRef,
}

impl Replica {
    pub fn new() -> Self {
        let doc = Doc::with_options(Options { offset_kind: OffsetKind::Utf16, ..Options::default() });
        let text = doc.get_or_insert_text(TEXT_NAME);
        Self { doc, text }
    }

    pub fn restore(blobs: &[Vec<u8>]) -> Result<Self, ReplicaError> {
        let replica = Self::new();
        for blob in blobs {
            replica.apply_update(blob, "restore")?;
        }
        Ok(replica)
    }

    pub fn text(&self) -> String {
        self.text.get_string(&self.doc.transact())
    }

    pub fn len(&self) -> usize {
        self.text.len(&self.doc.transact()) as usize
    }

    pub fn apply_update(&self, update: &[u8], origin: &str) -> Result<(), ReplicaError> {
        let update = Update::decode_v1(update).map_err(failure)?;
        self.doc.transact_mut_with(origin).apply_update(update).map_err(failure)
    }

    pub fn apply_edits(&self, edits: &[TextEdit], origin: &str) -> Vec<u8> {
        let mut transaction = self.doc.transact_mut_with(origin);
        for edit in edits.iter().rev() {
            if edit.to > edit.from {
                self.text.remove_range(&mut transaction, edit.from as u32, (edit.to - edit.from) as u32);
            }
            if !edit.insert.is_empty() {
                self.text.insert(&mut transaction, edit.from as u32, &edit.insert);
            }
        }
        transaction.encode_update_v1()
    }

    pub fn encode_position(&self, index: usize) -> Option<Vec<u8>> {
        let transaction = self.doc.transact();
        let index = index.min(self.text.len(&transaction) as usize) as u32;
        self.text.sticky_index(&transaction, index, Assoc::After).map(|position| position.encode_v1())
    }

    pub fn resolve_position(&self, encoded: &[u8]) -> Option<usize> {
        let position = StickyIndex::decode_v1(encoded).ok()?;
        let transaction = self.doc.transact();
        let offset = position.get_offset(&transaction)?;
        let text: &Branch = self.text.as_ref();
        (offset.branch == BranchPtr::from(text)).then_some(offset.index as usize)
    }

    pub fn state(&self) -> Vec<u8> {
        self.doc.transact().encode_state_as_update_v1(&StateVector::default())
    }
}

#[cfg(test)]
#[path = "replica_tests.rs"]
mod tests;
