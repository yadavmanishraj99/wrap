//! Downloading songs for offline listening.
//!
//! A download is one audio file fetched from the song's Saavn
//! download URL and saved under the user's music folder. A single
//! song and a whole collection (album / playlist / chart) are both
//! supported: a collection download is one entry per song, grouped
//! under the collection id, so the UI can show the collection's
//! overall progress as well as each song's own progress. A song
//! downloaded on its own belongs to no collection, so only that
//! song shows progress.
//!
//! Resilience: every download writes to a `.part` file and resumes
//! from it (HTTP `Range`) after a failure or an app restart, so a
//! flaky network does not lose what is already on disk. A failed
//! attempt is retried a few times with a backoff before it is
//! marked failed. The queue (items + collections) is persisted to
//! `downloads.json` under the app folder, so an interrupted
//! download resumes on the next launch.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// How many times a failed attempt is retried before the item is
/// marked failed.
const MAX_ATTEMPTS: u32 = 3;

/// Pause between retries (multiplied by the attempt number).
const RETRY_BASE_DELAY: Duration = Duration::from_secs(1);

/// Progress events are sent to the Shell at most this often per
/// item, so the UI is not flooded with a message per chunk.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Completed,
    Failed,
}

/// One song's download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    /// The song id (the key).
    pub song_id: String,
    pub title: String,
    pub artist: String,
    /// The collection this download belongs to, when the song was
    /// downloaded as part of an album / playlist / chart. `None`
    /// for a song downloaded on its own.
    pub collection_id: Option<String>,
    /// The direct audio URL (from the song's download links).
    pub url: String,
    /// Total bytes when the server reported a length.
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub status: DownloadStatus,
    pub error: Option<String>,
    /// The file name (sanitized), e.g. `Artist - Title.mp3`.
    pub file_name: String,
    /// Subfolder under the download dir, e.g. the album title for
    /// a collection download. Empty for a lone song.
    pub subfolder: String,
    /// The song's art URL (for the Downloads playlist cards).
    pub art: Option<String>,
    /// The album name, when known (for offline browsing).
    pub album: Option<String>,
    /// The song length in seconds, when known.
    pub duration: Option<u64>,
    /// The locally cached art image path, when the art has been
    /// downloaded (so the art shows offline).
    pub local_image: Option<String>,
}

impl DownloadItem {
    /// Rebuild a `Song` from the stored download, for offline use
    /// (a downloaded collection's song list, the Downloads
    /// playlist). The art prefers the locally cached image (a
    /// `file://` URI) so it shows offline; the audio URL is empty
    /// because the player plays the local file instead.
    pub fn to_song(&self) -> saavnapi::Song {
        let image_url = match &self.local_image {
            Some(path) => file_uri(std::path::Path::new(path)),
            None => self.art.clone().unwrap_or_default(),
        };
        saavnapi::Song {
            id: self.song_id.clone(),
            name: self.title.clone(),
            kind: "song".to_string(),
            year: None,
            release_date: None,
            duration: self.duration,
            label: None,
            explicit_content: false,
            play_count: None,
            language: String::new(),
            has_lyrics: false,
            lyrics_id: None,
            url: String::new(),
            copyright: None,
            album: saavnapi::SongAlbum {
                id: None,
                name: self.album.clone(),
                url: None,
            },
            artists: saavnapi::ArtistGroups {
                primary: vec![saavnapi::ArtistMap {
                    id: String::new(),
                    name: self.artist.clone(),
                    role: String::new(),
                    kind: "artist".to_string(),
                    image: Vec::new(),
                    url: String::new(),
                }],
                featured: Vec::new(),
                all: Vec::new(),
            },
            image: vec![saavnapi::DownloadLink {
                quality: "500x500".to_string(),
                url: image_url,
            }],
            download_url: Vec::new(),
            encrypted_media_url: None,
        }
    }
}

/// One collection's download (an album, playlist or chart).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionDownload {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub total_items: usize,
    pub completed_items: usize,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub status: DownloadStatus,
    /// The collection's art URL (for the Library card). For an
    /// artist download this is the artist's picture; the top songs
    /// are grouped into a playlist under the artist's name.
    pub art: Option<String>,
}

/// Events a download task sends to the Shell.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Progress {
        song_id: String,
        downloaded: u64,
        total: Option<u64>,
    },
    Completed {
        song_id: String,
        /// The locally cached art image path, when the art was
        /// downloaded (so the art shows offline).
        local_image: Option<String>,
    },
    Failed {
        song_id: String,
        error: String,
    },
}

/// The download queue + state, owned by the Shell.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DownloadManager {
    /// By song id.
    pub items: HashMap<String, DownloadItem>,
    /// By collection id.
    pub collections: HashMap<String, CollectionDownload>,
    /// Which songs belong to each collection, in order (by
    /// collection id). This is what makes a downloaded album /
    /// playlist / artist's song list available offline.
    #[serde(default)]
    pub collection_songs: HashMap<String, Vec<String>>,
    /// Where files go (not persisted — set at startup).
    #[serde(skip)]
    pub download_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(download_dir: PathBuf) -> Self {
        Self {
            items: HashMap::new(),
            collections: HashMap::new(),
            collection_songs: HashMap::new(),
            download_dir,
        }
    }

    /// True when this song has a completed download on disk.
    pub fn is_downloaded(&self, song_id: &str) -> bool {
        self.items
            .get(song_id)
            .is_some_and(|item| item.status == DownloadStatus::Completed)
    }

    /// True when this song is currently downloading (or queued).
    pub fn is_downloading(&self, song_id: &str) -> bool {
        self.items.get(song_id).is_some_and(|item| {
            matches!(
                item.status,
                DownloadStatus::Queued | DownloadStatus::Downloading
            )
        })
    }

    /// (downloaded, total) for a song, when it has a download.
    pub fn item_progress(&self, song_id: &str) -> Option<(u64, Option<u64>)> {
        self.items
            .get(song_id)
            .map(|item| (item.downloaded_bytes, item.total_bytes))
    }

    /// A 0.0–1.0 fraction for a song, when its total is known.
    pub fn item_fraction(&self, song_id: &str) -> Option<f64> {
        let (downloaded, total) = self.item_progress(song_id)?;
        let total = total?;
        if total == 0 {
            return None;
        }
        Some((downloaded as f64 / total as f64).clamp(0.0, 1.0))
    }

    /// The individually downloaded songs (no collection) — the
    /// auto "Downloads" playlist in the Library. Collection
    /// downloads are not included; those stay with their album /
    /// playlist.
    pub fn downloaded_songs(&self) -> Vec<&DownloadItem> {
        self.items
            .values()
            .filter(|item| item.collection_id.is_none() && item.status == DownloadStatus::Completed)
            .collect()
    }

    /// The downloaded artist collections — each one is the
    /// artist's top songs, grouped into a playlist under the
    /// artist's name in the Library.
    pub fn downloaded_artists(&self) -> Vec<&CollectionDownload> {
        self.collections
            .values()
            .filter(|c| c.kind == "artist" && c.status == DownloadStatus::Completed)
            .collect()
    }

    /// A collection's download, if one exists.
    pub fn collection(&self, collection_id: &str) -> Option<&CollectionDownload> {
        self.collections.get(collection_id)
    }

    /// True when a collection is currently downloading (or queued).
    pub fn is_collection_downloading(&self, collection_id: &str) -> bool {
        self.collections.get(collection_id).is_some_and(|c| {
            matches!(
                c.status,
                DownloadStatus::Queued | DownloadStatus::Downloading
            )
        })
    }

    /// A 0.0–1.0 fraction for a collection, from its completed
    /// items (falls back to bytes when no items are done yet).
    pub fn collection_fraction(&self, collection_id: &str) -> Option<f64> {
        let c = self.collections.get(collection_id)?;
        if c.total_items == 0 {
            return None;
        }
        Some((c.completed_items as f64 / c.total_items as f64).clamp(0.0, 1.0))
    }

    /// The on-disk path of a finished download, if it exists.
    pub fn file_path(&self, item: &DownloadItem) -> PathBuf {
        let mut path = self.download_dir.clone();
        if !item.subfolder.is_empty() {
            path.push(&item.subfolder);
        }
        path.push(&item.file_name);
        path
    }

    /// The `.part` path used while a download is in progress.
    pub fn part_path(&self, item: &DownloadItem) -> PathBuf {
        let mut path = self.file_path(item);
        let name = format!(
            "{}.part",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        );
        path.set_file_name(name);
        path
    }

    /// Persist the queue to the SQLite database (`resonance.db`
    /// under the app folder).
    pub fn save(&self, app_dir: &std::path::Path) {
        let Ok(mut conn) = open_db(app_dir) else {
            return;
        };
        let Ok(tx) = conn.transaction() else {
            return;
        };
        for item in self.items.values() {
            let _ = tx.execute(
                "INSERT OR REPLACE INTO downloads
                 (song_id, title, artist, collection_id, url, total_bytes,
                  downloaded_bytes, status, error, file_name, subfolder, art,
                  album, duration, local_image)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                         ?13, ?14, ?15)",
                rusqlite::params![
                    item.song_id,
                    item.title,
                    item.artist,
                    item.collection_id,
                    item.url,
                    item.total_bytes.map(|n| n as i64),
                    item.downloaded_bytes as i64,
                    status_str(item.status),
                    item.error,
                    item.file_name,
                    item.subfolder,
                    item.art,
                    item.album,
                    item.duration.map(|n| n as i64),
                    item.local_image,
                ],
            );
        }
        for collection in self.collections.values() {
            let _ = tx.execute(
                "INSERT OR REPLACE INTO collections
                 (id, kind, title, total_items, completed_items,
                  total_bytes, downloaded_bytes, status, art)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    collection.id,
                    collection.kind,
                    collection.title,
                    collection.total_items as i64,
                    collection.completed_items as i64,
                    collection.total_bytes as i64,
                    collection.downloaded_bytes as i64,
                    status_str(collection.status),
                    collection.art,
                ],
            );
        }
        // The collection -> songs mapping (which songs belong to
        // each downloaded album / playlist / artist), so a
        // downloaded collection's song list is available offline.
        for (collection_id, song_ids) in &self.collection_songs {
            for (position, song_id) in song_ids.iter().enumerate() {
                let _ = tx.execute(
                    "INSERT OR REPLACE INTO collection_songs
                     (collection_id, song_id, position) VALUES (?1, ?2, ?3)",
                    rusqlite::params![collection_id, song_id, position as i64],
                );
            }
        }
        let _ = tx.commit();
    }

    /// Load the persisted queue from the SQLite database, if any.
    pub fn load(app_dir: &std::path::Path, download_dir: PathBuf) -> Self {
        let mut manager = Self::new(download_dir);
        let Ok(conn) = open_db(app_dir) else {
            return manager;
        };
        for mut item in load_items(&conn) {
            // Anything left mid-download when the app closed goes
            // back to Queued so it resumes on launch.
            if matches!(
                item.status,
                DownloadStatus::Queued | DownloadStatus::Downloading
            ) {
                item.status = DownloadStatus::Queued;
            }
            manager.items.insert(item.song_id.clone(), item);
        }
        for mut collection in load_collections(&conn) {
            if matches!(
                collection.status,
                DownloadStatus::Queued | DownloadStatus::Downloading
            ) {
                collection.status = DownloadStatus::Queued;
            }
            manager
                .collections
                .insert(collection.id.clone(), collection);
        }
        manager.collection_songs = load_collection_songs(&conn);
        manager
    }

    /// The downloaded songs of a collection, in order, from the
    /// stored mapping (available offline).
    pub fn collection_song_ids(&self, collection_id: &str) -> Vec<String> {
        self.collection_songs
            .get(collection_id)
            .cloned()
            .unwrap_or_default()
    }
}

/// All downloaded items in the database (empty when the table is
/// missing or unreadable).
fn load_items(conn: &rusqlite::Connection) -> Vec<DownloadItem> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT song_id, title, artist, collection_id, url,
                total_bytes, downloaded_bytes, status, error,
                file_name, subfolder, art, album, duration, local_image
                FROM downloads",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok(DownloadItem {
            song_id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            collection_id: row.get(3)?,
            url: row.get(4)?,
            total_bytes: row.get::<_, Option<i64>>(5)?.map(|n| n as u64),
            downloaded_bytes: row.get::<_, i64>(6)? as u64,
            status: status_from_str(&row.get::<_, String>(7)?),
            error: row.get(8)?,
            file_name: row.get(9)?,
            subfolder: row.get(10)?,
            art: row.get(11)?,
            album: row.get(12)?,
            duration: row.get::<_, Option<i64>>(13)?.map(|n| n as u64),
            local_image: row.get(14)?,
        })
    }) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

/// The collection -> songs mapping (which songs belong to each
/// downloaded collection, in order).
fn load_collection_songs(conn: &rusqlite::Connection) -> HashMap<String, Vec<String>> {
    let mut mapping: HashMap<String, Vec<String>> = HashMap::new();
    let Ok(mut stmt) =
        conn.prepare("SELECT collection_id, song_id FROM collection_songs ORDER BY position")
    else {
        return mapping;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return mapping;
    };
    for (collection_id, song_id) in rows.flatten() {
        mapping.entry(collection_id).or_default().push(song_id);
    }
    mapping
}

/// All downloaded collections in the database.
fn load_collections(conn: &rusqlite::Connection) -> Vec<CollectionDownload> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, kind, title, total_items, completed_items,
                total_bytes, downloaded_bytes, status, art FROM collections",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok(CollectionDownload {
            id: row.get(0)?,
            kind: row.get(1)?,
            title: row.get(2)?,
            total_items: row.get::<_, i64>(3)? as usize,
            completed_items: row.get::<_, i64>(4)? as usize,
            total_bytes: row.get::<_, i64>(5)? as u64,
            downloaded_bytes: row.get::<_, i64>(6)? as u64,
            status: status_from_str(&row.get::<_, String>(7)?),
            art: row.get(8)?,
        })
    }) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

/// The SQLite database file under the app folder.
pub fn db_path(app_dir: &std::path::Path) -> PathBuf {
    app_dir.join("resonance.db")
}

/// Open (creating it first) the app's SQLite database and make
/// sure the download tables exist.
pub fn open_db(app_dir: &std::path::Path) -> Result<rusqlite::Connection, rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path(app_dir))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS downloads (
            song_id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            collection_id TEXT,
            url TEXT NOT NULL,
            total_bytes INTEGER,
            downloaded_bytes INTEGER NOT NULL,
            status TEXT NOT NULL,
            error TEXT,
            file_name TEXT NOT NULL,
            subfolder TEXT NOT NULL,
            art TEXT,
            album TEXT,
            duration INTEGER,
            local_image TEXT
        );
        CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            total_items INTEGER NOT NULL,
            completed_items INTEGER NOT NULL,
            total_bytes INTEGER NOT NULL,
            downloaded_bytes INTEGER NOT NULL,
            status TEXT NOT NULL,
            art TEXT
        );
        CREATE TABLE IF NOT EXISTS collection_songs (
            collection_id TEXT NOT NULL,
            song_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            PRIMARY KEY (collection_id, song_id)
        );",
    )?;
    Ok(conn)
}

fn status_str(status: DownloadStatus) -> &'static str {
    match status {
        DownloadStatus::Queued => "Queued",
        DownloadStatus::Downloading => "Downloading",
        DownloadStatus::Completed => "Completed",
        DownloadStatus::Failed => "Failed",
    }
}

fn status_from_str(value: &str) -> DownloadStatus {
    match value {
        "Downloading" => DownloadStatus::Downloading,
        "Completed" => DownloadStatus::Completed,
        "Failed" => DownloadStatus::Failed,
        _ => DownloadStatus::Queued,
    }
}

/// The user's music folder + `Resonance`, e.g.
/// `%USERPROFILE%\Music\Resonance`. Falls back to the app folder
/// when the profile path cannot be found.
pub fn default_download_dir(app_dir: &std::path::Path) -> PathBuf {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let dir = PathBuf::from(profile).join("Music").join("Resonance");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    app_dir.join("downloads")
}

/// Make a string safe to use as a file or folder name.
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 32 => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The file name for a song: its ID (`<song_id>.mp3`). Files are
/// named by ID, not by artist/title — two different songs can
/// share an artist and a title, and their files must not collide.
pub fn song_file_name(song_id: &str) -> String {
    format!("{}.mp3", sanitize_filename(song_id))
}

/// A `file://` URI for a local path, so a `MediaSource` or an
/// `Image` can load a downloaded file or cached art. Each path
/// segment is percent-encoded (spaces become `%20` and so on);
/// backslashes become forward slashes.
pub fn file_uri(path: &std::path::Path) -> String {
    let mut uri = String::from("file:///");
    for (i, part) in path.components().enumerate() {
        let part = part.as_os_str().to_string_lossy();
        if i > 0 {
            uri.push('/');
        }
        for ch in part.chars() {
            match ch {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | ':' => {
                    uri.push(ch);
                }
                _ => {
                    let mut buf = [0u8; 4];
                    for byte in ch.encode_utf8(&mut buf).as_bytes() {
                        uri.push_str(&format!("%{byte:02X}"));
                    }
                }
            }
        }
    }
    uri
}

/// The locally cached art image for a song, under
/// `<download_dir>/covers/<song_id>.jpg`.
pub fn image_cache_path(download_dir: &std::path::Path, song_id: &str) -> PathBuf {
    download_dir.join("covers").join(format!("{song_id}.jpg"))
}

/// Download a song's art image into the local cache, so the art
/// shows offline. Returns the local path on success, `None` when
/// there is no art URL or the download fails (art is best-effort).
async fn cache_image(item: &DownloadItem, download_dir: &std::path::Path) -> Option<String> {
    let url = item.art.as_ref()?;
    let path = image_cache_path(download_dir, &item.song_id);
    if path.exists() {
        return Some(path.to_string_lossy().into_owned());
    }
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let response = reqwest::Client::new().get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if tokio::fs::write(&path, &bytes).await.is_err() {
        return None;
    }
    Some(path.to_string_lossy().into_owned())
}

/// Download one song, resuming a partial file and retrying on
/// network failure. Sends progress / completion / failure events
/// to the Shell. Returns `Ok(())` on success.
pub async fn download_song(
    item: DownloadItem,
    download_dir: PathBuf,
    events: mpsc::Sender<DownloadEvent>,
) -> Result<(), String> {
    let manager = DownloadManager::new(download_dir);
    let final_path = manager.file_path(&item);
    let part_path = manager.part_path(&item);
    if let Some(parent) = final_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let client = reqwest::Client::new();
    let mut downloaded: u64 = 0;
    // Resume from a partial file when one exists.
    if let Ok(meta) = tokio::fs::metadata(&part_path).await {
        downloaded = meta.len();
    }
    let mut total: Option<u64> = item.total_bytes;
    let mut last_progress = tokio::time::Instant::now();

    for attempt in 1..=MAX_ATTEMPTS {
        let mut request = client.get(&item.url);
        if downloaded > 0 {
            request = request.header("Range", format!("bytes={downloaded}-"));
        }
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    let _ = events
                        .send(DownloadEvent::Failed {
                            song_id: item.song_id.clone(),
                            error: format!("Network error: {err}"),
                        })
                        .await;
                    return Err(format!("Network error: {err}"));
                }
                tokio::time::sleep(RETRY_BASE_DELAY * attempt).await;
                continue;
            }
        };

        // A 206 means the server honoured the Range resume; a 200
        // means it ignored it, so start from scratch.
        let status = response.status();
        if status.as_u16() == 200 && downloaded > 0 {
            downloaded = 0;
            let _ = tokio::fs::remove_file(&part_path).await;
        }
        if total.is_none() {
            total = response.content_length().map(|len| len + downloaded);
        }
        if !status.is_success() {
            if attempt == MAX_ATTEMPTS {
                let message = format!("Server error: {status}");
                let _ = events
                    .send(DownloadEvent::Failed {
                        song_id: item.song_id.clone(),
                        error: message.clone(),
                    })
                    .await;
                return Err(message);
            }
            tokio::time::sleep(RETRY_BASE_DELAY * attempt).await;
            continue;
        }

        // Stream the body into the partial file, appending.
        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&part_path)
            .await
        {
            Ok(file) => file,
            Err(err) => {
                let message = format!("Cannot write file: {err}");
                let _ = events
                    .send(DownloadEvent::Failed {
                        song_id: item.song_id.clone(),
                        error: message.clone(),
                    })
                    .await;
                return Err(message);
            }
        };
        let mut failed: Option<String> = None;
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if file.write_all(&chunk).await.is_err() {
                        failed = Some("Cannot write file".to_string());
                        break;
                    }
                    downloaded += chunk.len() as u64;
                    if last_progress.elapsed() >= PROGRESS_THROTTLE {
                        last_progress = tokio::time::Instant::now();
                        let _ = events
                            .send(DownloadEvent::Progress {
                                song_id: item.song_id.clone(),
                                downloaded,
                                total,
                            })
                            .await;
                    }
                }
                Ok(None) => break,
                Err(err) => {
                    failed = Some(format!("Network error: {err}"));
                    break;
                }
            }
        }
        // Flush what was written before deciding.
        let _ = file.flush().await;
        drop(file);
        if let Some(err) = failed {
            if attempt == MAX_ATTEMPTS {
                let _ = events
                    .send(DownloadEvent::Failed {
                        song_id: item.song_id.clone(),
                        error: err.clone(),
                    })
                    .await;
                return Err(err);
            }
            tokio::time::sleep(RETRY_BASE_DELAY * attempt).await;
            continue;
        }

        // Done — move the partial file into place.
        if tokio::fs::rename(&part_path, &final_path).await.is_err() {
            // Cross-device rename can fail; copy instead.
            if tokio::fs::copy(&part_path, &final_path).await.is_err() {
                let message = "Cannot save file".to_string();
                let _ = events
                    .send(DownloadEvent::Failed {
                        song_id: item.song_id.clone(),
                        error: message.clone(),
                    })
                    .await;
                return Err(message);
            }
            let _ = tokio::fs::remove_file(&part_path).await;
        }
        // Cache the art image alongside, so it shows offline.
        let local_image = cache_image(&item, &manager.download_dir).await;
        let _ = events
            .send(DownloadEvent::Progress {
                song_id: item.song_id.clone(),
                downloaded,
                total,
            })
            .await;
        let _ = events
            .send(DownloadEvent::Completed {
                song_id: item.song_id.clone(),
                local_image,
            })
            .await;
        return Ok(());
    }
    Err("Download failed".to_string())
}
