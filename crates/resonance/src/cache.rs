//! A file-based data cache for pages — the offline-first layer.
//!
//! Home data, details pages and search results are cached as
//! JSON files under `%LOCALAPPDATA%\Resonance\cache\`, one file
//! per entry, each stamped with when it was fetched. A page
//! opens from its cache instantly (no skeleton, no network),
//! and a background refresh replaces the entry when it lands.
//!
//! Expiry is a *freshness* hint only: a fresh entry is served
//! without any network call, a stale entry is served too and a
//! refresh is attempted. Entries are only ever replaced by a
//! successful fetch — nothing is evicted while offline (there
//! is no eviction at all), so whatever was cached is whatever
//! the user can still browse and play from with no connection.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Home changes fast (trending), so it goes stale quickly.
pub const HOME_TTL_MS: i64 = 30 * 60 * 1000;
/// A details page (an album's tracklist, an artist's top
/// songs) barely changes — a day is fine.
pub const DETAILS_TTL_MS: i64 = 24 * 60 * 60 * 1000;
/// Search results are as volatile as Home.
pub const SEARCH_TTL_MS: i64 = 30 * 60 * 1000;

#[derive(Serialize, Deserialize)]
struct Entry {
    fetched_at: i64,
    payload: serde_json::Value,
}

/// The cache: a directory of entry files. Cheap to clone (just
/// a path); every read/write hits the disk directly.
#[derive(Debug, Clone, Default)]
pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn load(app_dir: &std::path::Path) -> Self {
        let dir = app_dir.join("cache");
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    /// The file for a key: a readable prefix (for debugging)
    /// plus a hash of the full key, so keys that differ only in
    /// punctuation or case-folded characters never collide.
    fn file_for(&self, key: &str) -> PathBuf {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in key.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        let prefix: String = key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .take(40)
            .collect();
        self.dir.join(format!("{prefix}-{hash:016x}.json"))
    }

    /// Read an entry. Returns the value and whether it is still
    /// fresh (younger than `ttl_ms`). A stale value is returned
    /// too — the caller serves it and refreshes in the
    /// background; only a missing or unreadable entry is `None`.
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str, ttl_ms: i64) -> Option<(T, bool)> {
        let text = std::fs::read_to_string(self.file_for(key)).ok()?;
        let entry: Entry = serde_json::from_str(&text).ok()?;
        let value: T = serde_json::from_value(entry.payload).ok()?;
        let fresh = crate::catalog::now_ms() - entry.fetched_at < ttl_ms;
        Some((value, fresh))
    }

    /// Store an entry (a successful fetch replacing whatever
    /// was there). Best-effort: a failed write just means the
    /// next open fetches again.
    pub fn put<T: Serialize>(&self, key: &str, value: &T) {
        let Ok(payload) = serde_json::to_value(value) else {
            return;
        };
        let entry = Entry {
            fetched_at: crate::catalog::now_ms(),
            payload,
        };
        if let Ok(text) = serde_json::to_string(&entry) {
            let _ = std::fs::write(self.file_for(key), text);
        }
    }
}

/// The cache key for the Home launch data.
pub fn home_key() -> &'static str {
    "home"
}

/// The cache key for one details page.
pub fn details_key(kind: &str, id: &str) -> String {
    format!("details:{kind}:{id}")
}

/// The cache key for one search (queries are case-insensitive
/// and trimmed, so "Arijit" and "arijit " share an entry).
pub fn search_key(query: &str) -> String {
    format!("search:{}", query.trim().to_lowercase())
}
