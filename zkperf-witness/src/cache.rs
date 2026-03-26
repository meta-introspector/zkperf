//! Witness cache: deduplicates by signature, aggregates stats.
//!
//! Storage: `~/.zkperf/cache/<signature>.json`
//!
//! Each cache entry tracks how many times a boundary was crossed,
//! min/max/avg elapsed, and the latest violation state.

use crate::{dirs_fallback, Witness};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub signature: String,
    pub context: String,
    pub complexity: String,
    pub max_n: u64,
    pub max_ms: u64,
    pub count: u64,
    pub min_ms: u64,
    pub max_elapsed_ms: u64,
    pub total_ms: u64,
    pub violation_count: u64,
    pub last_timestamp: u64,
}

impl CacheEntry {
    pub fn avg_ms(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.total_ms / self.count
        }
    }
}

fn cache_dir() -> PathBuf {
    dirs_fallback().join("cache")
}

fn entry_path(signature: &str) -> PathBuf {
    cache_dir().join(format!("{}.json", &signature[..16.min(signature.len())]))
}

/// Update cache for a recorded witness. Returns the updated entry.
pub fn update(w: &Witness) -> Option<CacheEntry> {
    let path = entry_path(w.signature);
    let mut entry = load_entry(&path).unwrap_or_else(|| CacheEntry {
        signature: w.signature.to_string(),
        context: w.context.to_string(),
        complexity: w.complexity.to_string(),
        max_n: w.max_n,
        max_ms: w.max_ms,
        count: 0,
        min_ms: u64::MAX,
        max_elapsed_ms: 0,
        total_ms: 0,
        violation_count: 0,
        last_timestamp: 0,
    });

    entry.count += 1;
    entry.min_ms = entry.min_ms.min(w.elapsed_ms);
    entry.max_elapsed_ms = entry.max_elapsed_ms.max(w.elapsed_ms);
    entry.total_ms += w.elapsed_ms;
    if w.violated {
        entry.violation_count += 1;
    }
    entry.last_timestamp = w.timestamp;

    save_entry(&path, &entry).ok()?;
    Some(entry)
}

/// Lookup cached stats for a signature.
pub fn lookup(signature: &str) -> Option<CacheEntry> {
    load_entry(&entry_path(signature))
}

/// List all cached entries.
pub fn list_all() -> Vec<CacheEntry> {
    let dir = cache_dir();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    rd.filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| load_entry(&e.path()))
        .collect()
}

fn load_entry(path: &PathBuf) -> Option<CacheEntry> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_entry(path: &PathBuf, entry: &CacheEntry) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let json = serde_json::to_string(entry).map_err(|e| std::io::Error::other(e))?;
    std::fs::write(path, json)
}
