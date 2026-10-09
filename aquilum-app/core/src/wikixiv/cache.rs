use blake3::Hash;
use std::collections::HashMap;
use super::models::WikixivHit;

const CACHE_CAP: usize = 32;

pub struct HitCache {
    order: Vec<Hash>,
    entries: HashMap<Hash, Vec<WikixivHit>>,
}

impl HitCache {
    pub fn new() -> Self {
        Self {
            order: Vec::new(),
            entries: HashMap::new(),
        }
    }

    pub fn get(&mut self, key: &Hash) -> Option<Vec<WikixivHit>> {
        let hits = self.entries.get(key)?.clone();
        if let Some(index) = self.order.iter().position(|item| item == key) {
            let hash = self.order.remove(index);
            self.order.push(hash);
        }
        Some(hits)
    }

    pub fn put(&mut self, key: Hash, hits: Vec<WikixivHit>) {
        if let Some(index) = self.order.iter().position(|item| item == &key) {
            self.order.remove(index);
        }
        self.entries.insert(key, hits);
        self.order.push(key);
        while self.order.len() > CACHE_CAP {
            if let Some(old) = self.order.first().copied() {
                self.order.remove(0);
                self.entries.remove(&old);
            } else {
                break;
            }
        }
    }
}

pub fn content_hash(document_path: Option<&str>, text: &str) -> Hash {
    let mut hasher = blake3::Hasher::new();
    if let Some(path) = document_path {
        hasher.update(path.as_bytes());
    }
    hasher.update(b"\0");
    hasher.update(text.as_bytes());
    hasher.finalize()
}
