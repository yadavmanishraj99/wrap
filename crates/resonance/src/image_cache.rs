//! A disk cache for album art, so Home and details pages do not
//! re-download the same image on every visit.
//!
//! Images are stored under `<app_dir>/image_cache/`, one file per
//! URL, named by a hash of the URL. The cache is bounded by the
//! image-cache size in Settings (default 500 MB); past the limit
//! the least-recently-used images are evicted. The read path is a
//! plain file-exists check (fast, synchronous), so the UI can ask
//! for a cached image while it builds. Downloads run in the
//! background, kicked off when a page's data arrives.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// The image cache folder.
pub fn cache_dir(app_dir: &Path) -> PathBuf {
    app_dir.join("image_cache")
}

/// The cached file for a URL (its name is a hash of the URL).
pub fn cached_path(cache_dir: &Path, url: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    cache_dir.join(format!("{:016x}.jpg", hasher.finish()))
}

/// The cached file for a URL, when it is already on disk.
/// `None` for a `file://` URL (already local) and for a URL that
/// has not been cached yet.
pub fn lookup(cache_dir: &Path, url: &str) -> Option<PathBuf> {
    if url.starts_with("file://") {
        return None;
    }
    let path = cached_path(cache_dir, url);
    if path.exists() { Some(path) } else { None }
}

/// Download an image into the cache (best-effort: a failure just
/// leaves the remote URL in use). Afterwards, evict the
/// least-recently-used images past the size limit.
pub async fn cache_image(url: &str, cache_dir: &Path, limit_mb: u64) {
    let path = cached_path(cache_dir, url);
    if path.exists() {
        touch(&path);
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let Ok(response) = reqwest::Client::new().get(url).send().await else {
        return;
    };
    if !response.status().is_success() {
        return;
    }
    let Ok(bytes) = response.bytes().await else {
        return;
    };
    if tokio::fs::write(&path, &bytes).await.is_err() {
        return;
    }
    evict(cache_dir, limit_mb);
}

/// Cache every art URL in the list, in the background.
pub async fn cache_images(urls: Vec<String>, cache_dir: PathBuf, limit_mb: u64) {
    for url in urls {
        cache_image(&url, &cache_dir, limit_mb).await;
    }
}

/// Refresh a cached file's last-used time (so it is not evicted
/// while it is still shown).
fn touch(path: &Path) {
    if let Ok(file) = std::fs::File::options().write(true).open(path) {
        let now = std::time::SystemTime::now();
        let _ = file.set_times(std::fs::FileTimes::new().set_modified(now));
    }
}

/// Drop the least-recently-used cached images until the cache is
/// under the size limit.
fn evict(cache_dir: &Path, limit_mb: u64) {
    let limit = limit_mb.saturating_mul(1024 * 1024);
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };
    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .flatten()
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let modified = meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            Some((entry.path(), meta.len(), modified))
        })
        .collect();
    let total: u64 = files.iter().map(|(_, size, _)| *size).sum();
    if total <= limit {
        return;
    }
    // Oldest first.
    files.sort_by_key(|(_, _, modified)| *modified);
    let mut total = total;
    for (path, size, _) in files {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}
