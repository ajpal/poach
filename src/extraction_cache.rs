//! A cache mapping extraction inputs (s-expressions) to their results.
//!
//! Used by the `poach` binary to memoize the results of `(extract ...)`
//! and `(multi-extract ...)` commands across runs.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExtractionCache {
    entries: HashMap<String, Entry>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Entry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    best: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variants: Vec<String>,
    /// True if a previous `insert_variants` asked for strictly more variants
    /// than the extractor returned — i.e., `variants` enumerates everything
    /// possible. Future lookups for any `n` may then be served from cache.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    exhausted: bool,
}

impl ExtractionCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let cache = serde_json::from_reader(file)?;
        Ok(cache)
    }

    /// Serialize to JSON, returning the number of bytes written.
    pub fn save(&self, path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, &bytes)?;
        Ok(bytes.len())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn lookup_best(&self, key: &str) -> Option<&str> {
        self.entries.get(key).and_then(|e| e.best.as_deref())
    }

    /// On a hit, returns up to `n` variants borrowed from the cache.
    pub fn lookup_variants(&self, key: &str, n: usize) -> Option<&[String]> {
        let entry = self.entries.get(key)?;
        if entry.variants.len() >= n || entry.exhausted {
            let take = n.min(entry.variants.len());
            Some(&entry.variants[..take])
        } else {
            None
        }
    }

    /// Record a best-extraction result. First write wins (subsequent calls are ignored)
    /// to keep training deterministic across reruns.
    pub fn insert_best(&mut self, key: String, term: String) {
        let entry = self.entries.entry(key).or_default();
        if entry.best.is_none() {
            entry.best = Some(term);
        }
    }

    /// Record a variants result. `n` is the count the command requested;
    /// if it strictly exceeded what came back, the entry is marked exhausted
    /// so future lookups for any `n` can be served from cache.
    pub fn insert_variants(&mut self, key: String, terms: Vec<String>, n: usize) {
        let entry = self.entries.entry(key).or_default();
        if terms.len() > entry.variants.len() {
            entry.variants = terms;
        }
        if n > entry.variants.len() {
            entry.exhausted = true;
        }
    }
}
