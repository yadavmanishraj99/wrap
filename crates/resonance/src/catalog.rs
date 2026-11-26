//! The catalog: every song and collection the app knows about,
//! plus the download engine that fetches songs for offline use.
//!
//! Storage model (SQLite, `resonance.db` under the app folder):
//!
//! - `media` — one row per song, ever: its metadata (title,
//!   artists, album, duration, art) and its download state. The
//!   audio file, when downloaded, is always `music/<id>.mp3`
//!   under the download folder — the path is derived from the id,
//!   never stored, and a song is downloaded at most once no
//!   matter how many collections contain it.
//! - `collections` — one row per album / playlist / chart /
//!   artist, plus the auto "Downloads" playlist: everything
//!   needed to reconstruct its details view offline (title,
//!   description, cover, artists, follower count).
//! - `collection_media` — the relationship: which media belong to
//!   which collection, with each collection's own track order.
//!   One media can belong to many collections.
//! - `library_refs` — liked / pinned / recent entries. Each ref
//!   just points at a media or collection row, so liking or
//!   pinning something creates its catalog row (not downloaded)
//!   and the ref on top of it.
//!
//! Every row carries `created_at` / `updated_at` (Unix
//! milliseconds). Insert sets both; every later change — a
//! rearrange, a re-requested download, a replay — touches
//! `updated_at` only. Ordering follows the timestamps: pinned
//! and recent sort by `updated_at`, newest first. Re-requesting
//! a download of an already-downloaded song is a touch, never a
//! second download.
//!
//! Covers live in `covers/<cover_id>.jpg` under the download
//! folder. Every song caches its own art under its own id, and
//! a downloaded collection (album / playlist / artist) also
//! downloads its OWN cover under the collection id — the
//! collection's art, not a member song's — so its page and
//! cards show the right picture offline.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::store::PinnedItem;

/// The id of the auto "Downloads" collection — every song
/// downloaded on its own joins it.
pub const DOWNLOADS_ID: &str = "downloads";

/// How many times a failed attempt is retried before the item is
/// marked failed.
const MAX_ATTEMPTS: u32 = 3;

/// Pause between retries (multiplied by the attempt number).
const RETRY_BASE_DELAY: Duration = Duration::from_secs(1);

/// Progress events are sent to the Shell at most this often per
/// item, so the UI is not flooded with a message per chunk.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

/// Now, as Unix milliseconds — the timestamp unit of every
/// `created_at` / `updated_at` in the catalog.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A media row's download state. `NotDownloaded` is a catalog
/// row that exists only because the song is liked / pinned /
/// recent / a collection member.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaStatus {
    NotDownloaded,
    Queued,
    Downloading,
    Completed,
    Failed,
}

impl MediaStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotDownloaded => "NotDownloaded",
            Self::Queued => "Queued",
            Self::Downloading => "Downloading",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "Queued" => Self::Queued,
            "Downloading" => Self::Downloading,
            "Completed" => Self::Completed,
            "Failed" => Self::Failed,
            _ => Self::NotDownloaded,
        }
    }
}

/// One song in the catalog: metadata + download state.
#[derive(Debug, Clone)]
pub struct MediaRow {
    pub id: String,
    pub title: String,
    /// The display artist line (comma-joined primary artists).
    pub artist: String,
    /// The full artist groups (for clickable artist links).
    pub artists: saavnapi::ArtistGroups,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub duration: Option<u64>,
    pub year: Option<String>,
    pub language: String,
    pub has_lyrics: bool,
    pub lyrics_id: Option<String>,
    /// The remote art URL.
    pub art: Option<String>,
    /// Which file in `covers/` is this song's art.
    pub cover_id: String,
    pub status: MediaStatus,
    /// The direct audio URL of an in-flight / resumable download.
    pub url: String,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl MediaRow {
    /// Build a catalog row from a Saavn song (metadata only, not
    /// downloaded). `cover_id` starts as the song's own id.
    fn from_song(song: &saavnapi::Song, now: i64) -> Self {
        Self {
            id: song.id.clone(),
            title: song.name.clone(),
            artist: artist_line(&song.artists),
            artists: song.artists.clone(),
            album: song.album.name.clone(),
            album_id: song.album.id.clone(),
            duration: song.duration,
            year: song.year.clone(),
            language: song.language.clone(),
            has_lyrics: song.has_lyrics,
            lyrics_id: song.lyrics_id.clone(),
            art: best_art(&song.image),
            cover_id: song.id.clone(),
            status: MediaStatus::NotDownloaded,
            url: String::new(),
            total_bytes: None,
            downloaded_bytes: 0,
            error: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Refresh the metadata from a fresher copy of the song. The
    /// download state and cover choice are never touched.
    fn refresh_from_song(&mut self, song: &saavnapi::Song) {
        self.title = song.name.clone();
        self.artist = artist_line(&song.artists);
        self.artists = song.artists.clone();
        self.album = song.album.name.clone();
        self.album_id = song.album.id.clone();
        self.duration = song.duration;
        self.year = song.year.clone();
        self.language = song.language.clone();
        self.has_lyrics = song.has_lyrics;
        self.lyrics_id = song.lyrics_id.clone();
        self.art = best_art(&song.image);
    }

    /// Rebuild a `Song` for the UI / player. The image prefers
    /// the locally cached cover (a `file://` URI) so it shows
    /// offline; the audio URL is empty because a downloaded song
    /// plays from its local file.
    pub fn to_song(&self, download_dir: &std::path::Path) -> saavnapi::Song {
        let cover = cover_path(download_dir, &self.cover_id);
        let image_url = if cover.exists() {
            file_uri(&cover)
        } else {
            self.art.clone().unwrap_or_default()
        };
        saavnapi::Song {
            id: self.id.clone(),
            name: self.title.clone(),
            kind: "song".to_string(),
            year: self.year.clone(),
            release_date: None,
            duration: self.duration,
            label: None,
            explicit_content: false,
            play_count: None,
            language: self.language.clone(),
            has_lyrics: self.has_lyrics,
            lyrics_id: self.lyrics_id.clone(),
            url: String::new(),
            copyright: None,
            album: saavnapi::SongAlbum {
                id: self.album_id.clone(),
                name: self.album.clone(),
                url: None,
            },
            artists: self.artists.clone(),
            image: vec![saavnapi::DownloadLink {
                quality: "500x500".to_string(),
                url: image_url,
            }],
            download_url: Vec::new(),
            encrypted_media_url: None,
        }
    }
}

/// One collection in the catalog: an album, playlist, chart or
/// artist, or the auto "Downloads" playlist — with everything
/// its details view needs.
#[derive(Debug, Clone)]
pub struct CollectionRow {
    pub id: String,
    /// "album" / "playlist" / "chart" / "artist" / "downloads".
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub description: Option<String>,
    pub year: Option<i64>,
    pub language: String,
    /// The remote art URL.
    pub art: Option<String>,
    /// Which file in `covers/` is this collection's art.
    pub cover_id: String,
    /// The artist groups (albums show artist links).
    pub artists: saavnapi::ArtistGroups,
    /// Follower count (artists only).
    pub followers: Option<u64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl CollectionRow {
    /// The header image URL for the UI: the local cover file when
    /// it exists (offline), else the remote art.
    fn image_url(&self, download_dir: &std::path::Path) -> String {
        let cover = cover_path(download_dir, &self.cover_id);
        if cover.exists() {
            file_uri(&cover)
        } else {
            self.art.clone().unwrap_or_default()
        }
    }

    fn image_links(&self, download_dir: &std::path::Path) -> Vec<saavnapi::DownloadLink> {
        vec![saavnapi::DownloadLink {
            quality: "500x500".to_string(),
            url: self.image_url(download_dir),
        }]
    }
}

/// One membership: a media inside a collection, at a position.
#[derive(Debug, Clone)]
pub struct MemberRow {
    pub media_id: String,
    pub position: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// One library reference: a liked / pinned / recent pointer at a
/// media or collection row.
#[derive(Debug, Clone)]
pub struct RefRow {
    /// "liked" / "pinned" / "recent".
    pub category: String,
    /// "media" / "collection".
    pub target_kind: String,
    pub target_id: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// The target of a download request (a song, or a collection to
/// download every song of), as the card / page described it.
pub struct TargetInfo<'a> {
    pub kind: &'a str,
    pub id: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub art: Option<&'a str>,
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
    },
    Failed {
        song_id: String,
        error: String,
    },
}

/// The catalog: all media, collections, memberships and library
/// refs, in memory and persisted to SQLite. Owned by the Shell.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    /// By song id.
    pub media: HashMap<String, MediaRow>,
    /// By collection id.
    pub collections: HashMap<String, CollectionRow>,
    /// Memberships by collection id (unordered; sort on read).
    pub members: HashMap<String, Vec<MemberRow>>,
    /// All library refs (small; filtered on read).
    pub refs: Vec<RefRow>,
    /// Where downloaded files live (not persisted).
    pub download_dir: PathBuf,
    /// The app folder holding the database (not persisted).
    pub app_dir: PathBuf,
}

/// The comma-joined primary artist names of a song (falling back
/// to featured, then all), or "Unknown artist".
pub fn artist_line(groups: &saavnapi::ArtistGroups) -> String {
    let names: Vec<&str> = if !groups.primary.is_empty() {
        groups.primary.iter().map(|a| a.name.as_str()).collect()
    } else if !groups.featured.is_empty() {
        groups.featured.iter().map(|a| a.name.as_str()).collect()
    } else {
        groups.all.iter().map(|a| a.name.as_str()).collect()
    };
    if names.is_empty() {
        "Unknown artist".to_string()
    } else {
        names.join(", ")
    }
}

/// The best art URL from a link list (500x500 when offered, else
/// the last link).
pub fn best_art(links: &[saavnapi::DownloadLink]) -> Option<String> {
    links
        .iter()
        .find(|link| link.quality == "500x500")
        .or_else(|| links.last())
        .map(|link| link.url.clone())
}

/// The audio file of a downloaded song: `music/<id>.mp3` under
/// the download folder.
pub fn music_path(download_dir: &std::path::Path, media_id: &str) -> PathBuf {
    download_dir
        .join("music")
        .join(format!("{}.mp3", sanitize_filename(media_id)))
}

/// The `.part` file a download in progress writes to.
pub fn part_path(download_dir: &std::path::Path, media_id: &str) -> PathBuf {
    download_dir
        .join("music")
        .join(format!("{}.mp3.part", sanitize_filename(media_id)))
}

/// The cover file for a cover id: `covers/<cover_id>.jpg`.
pub fn cover_path(download_dir: &std::path::Path, cover_id: &str) -> PathBuf {
    download_dir
        .join("covers")
        .join(format!("{}.jpg", sanitize_filename(cover_id)))
}

impl Catalog {
    /// The audio file of a downloaded song.
    pub fn music_file(&self, media_id: &str) -> PathBuf {
        music_path(&self.download_dir, media_id)
    }

    /// A `file://` URI for a song's local cover, when the cover
    /// file exists (so the art shows offline).
    pub fn cover_uri(&self, cover_id: &str) -> Option<String> {
        let path = cover_path(&self.download_dir, cover_id);
        path.exists().then(|| file_uri(&path))
    }
}

impl Catalog {
    // ---------- download state queries ----------

    /// True when this song has a completed download on disk.
    pub fn is_downloaded(&self, song_id: &str) -> bool {
        self.media
            .get(song_id)
            .is_some_and(|row| row.status == MediaStatus::Completed)
    }

    /// True when this song is currently downloading (or queued).
    pub fn is_downloading(&self, song_id: &str) -> bool {
        self.media
            .get(song_id)
            .is_some_and(|row| matches!(row.status, MediaStatus::Queued | MediaStatus::Downloading))
    }

    /// A 0.0–1.0 download fraction for a song, when its total
    /// size is known.
    pub fn item_fraction(&self, song_id: &str) -> Option<f64> {
        let row = self.media.get(song_id)?;
        let total = row.total_bytes?;
        if total == 0 {
            return None;
        }
        Some((row.downloaded_bytes as f64 / total as f64).clamp(0.0, 1.0))
    }

    /// A collection's member media ids, in the collection's own
    /// track order.
    pub fn collection_song_ids(&self, collection_id: &str) -> Vec<String> {
        let mut members: Vec<&MemberRow> = self
            .members
            .get(collection_id)
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        members.sort_by_key(|m| m.position);
        members.into_iter().map(|m| m.media_id.clone()).collect()
    }

    /// (completed, total) member counts for a collection.
    pub fn collection_progress(&self, collection_id: &str) -> Option<(usize, usize)> {
        let members = self.members.get(collection_id)?;
        let completed = members
            .iter()
            .filter(|m| {
                self.media
                    .get(&m.media_id)
                    .is_some_and(|row| row.status == MediaStatus::Completed)
            })
            .count();
        Some((completed, members.len()))
    }

    /// True when every member of the collection is downloaded.
    pub fn collection_complete(&self, collection_id: &str) -> bool {
        self.collection_progress(collection_id)
            .is_some_and(|(completed, total)| total > 0 && completed == total)
    }

    /// True when any member of the collection is downloading.
    pub fn is_collection_downloading(&self, collection_id: &str) -> bool {
        self.members
            .get(collection_id)
            .is_some_and(|members| members.iter().any(|m| self.is_downloading(&m.media_id)))
    }

    /// A 0.0–1.0 fraction for a collection, from its completed
    /// members.
    pub fn collection_fraction(&self, collection_id: &str) -> Option<f64> {
        let (completed, total) = self.collection_progress(collection_id)?;
        if total == 0 {
            return None;
        }
        Some((completed as f64 / total as f64).clamp(0.0, 1.0))
    }

    /// The individually downloaded songs — the members of the
    /// auto "Downloads" collection that are completed, most
    /// recently touched first.
    /// A downloaded song rebuilt for the player, when the song
    /// is in the catalog and its file is on disk. This is what
    /// makes a pinned / library / card song play offline: no
    /// network fetch, the player takes the local file.
    pub fn downloaded_song(&self, song_id: &str) -> Option<saavnapi::Song> {
        let row = self.media.get(song_id)?;
        if row.status != MediaStatus::Completed {
            return None;
        }
        Some(row.to_song(&self.download_dir))
    }

    /// The playable (downloaded) songs for a play request: for
    /// a song, itself when downloaded; for a collection, its
    /// downloaded members in track order. Empty when nothing
    /// playable is stored — the caller then tries the network
    /// (or reports that it cannot play offline).
    pub fn playable_songs(&self, kind: &str, id: &str) -> Vec<saavnapi::Song> {
        if kind == "song" {
            return self.downloaded_song(id).into_iter().collect();
        }
        self.collection_song_ids(id)
            .iter()
            .filter_map(|song_id| self.downloaded_song(song_id))
            .collect()
    }

    /// Search the local catalog — the offline search. Matches
    /// media and collections by title / artist text and returns
    /// them in the `search_all` shape, so the Search page
    /// renders them like live results (downloaded songs first,
    /// since those are the ones that can actually play).
    pub fn search_local(&self, query: &str) -> saavnapi::SearchAll {
        let needle = query.trim().to_lowercase();
        let matches = |text: &str| !needle.is_empty() && text.to_lowercase().contains(&needle);
        let image_of = |art: &Option<String>, cover_id: &str| -> Vec<saavnapi::DownloadLink> {
            let url = self
                .cover_uri(cover_id)
                .or_else(|| art.clone())
                .unwrap_or_default();
            vec![saavnapi::DownloadLink {
                quality: "500x500".to_string(),
                url,
            }]
        };

        let mut media: Vec<&MediaRow> = self
            .media
            .values()
            .filter(|row| matches(&row.title) || matches(&row.artist))
            .collect();
        // Downloaded first (playable offline), then most
        // recently touched.
        media.sort_by(|a, b| {
            let rank = |row: &MediaRow| row.status != MediaStatus::Completed;
            rank(a).cmp(&rank(b)).then(b.updated_at.cmp(&a.updated_at))
        });
        let songs: Vec<saavnapi::SearchHit> = media
            .iter()
            .take(20)
            .map(|row| saavnapi::SearchHit {
                id: row.id.clone(),
                title: row.title.clone(),
                image: image_of(&row.art, &row.cover_id),
                album: row.album.clone(),
                url: None,
                kind: Some("song".to_string()),
                language: Some(row.language.clone()),
                description: None,
                primary_artists: Some(row.artist.clone()),
                singers: None,
            })
            .collect();

        let mut collections: Vec<&CollectionRow> = self
            .collections
            .values()
            .filter(|row| row.id != DOWNLOADS_ID)
            .filter(|row| matches(&row.title) || matches(&row.subtitle))
            .collect();
        collections.sort_by_key(|row| std::cmp::Reverse(row.updated_at));
        let mut albums: Vec<saavnapi::SearchAlbumHit> = Vec::new();
        let mut artists: Vec<saavnapi::SearchArtistHit> = Vec::new();
        let mut playlists: Vec<saavnapi::SearchPlaylistHit> = Vec::new();
        for row in collections.into_iter().take(20) {
            match row.kind.as_str() {
                "album" => albums.push(saavnapi::SearchAlbumHit {
                    id: row.id.clone(),
                    title: row.title.clone(),
                    image: image_of(&row.art, &row.cover_id),
                    artist: Some(artist_line(&row.artists)),
                    url: None,
                    kind: Some("album".to_string()),
                    description: row.description.clone(),
                    year: row.year.map(|y| y.to_string()),
                    song_ids: None,
                    language: Some(row.language.clone()),
                }),
                "artist" => artists.push(saavnapi::SearchArtistHit {
                    id: row.id.clone(),
                    title: row.title.clone(),
                    image: image_of(&row.art, &row.cover_id),
                    kind: Some("artist".to_string()),
                    description: Some(row.subtitle.clone()),
                    position: None,
                }),
                _ => playlists.push(saavnapi::SearchPlaylistHit {
                    id: row.id.clone(),
                    title: row.title.clone(),
                    image: image_of(&row.art, &row.cover_id),
                    url: None,
                    kind: Some(row.kind.clone()),
                    language: Some(row.language.clone()),
                    description: row.description.clone(),
                }),
            }
        }

        let top = songs.first().cloned().into_iter().collect();
        saavnapi::SearchAll {
            top_query: saavnapi::SearchSection {
                results: top,
                position: 0,
            },
            songs: saavnapi::SearchSection {
                results: songs,
                position: 1,
            },
            albums: saavnapi::SearchSection {
                results: albums,
                position: 2,
            },
            artists: saavnapi::SearchSection {
                results: artists,
                position: 3,
            },
            playlists: saavnapi::SearchSection {
                results: playlists,
                position: 4,
            },
        }
    }

    pub fn downloaded_songs(&self) -> Vec<MediaRow> {
        let mut members: Vec<&MemberRow> = self
            .members
            .get(DOWNLOADS_ID)
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        members.sort_by_key(|m| std::cmp::Reverse(m.updated_at));
        members
            .into_iter()
            .filter_map(|m| self.media.get(&m.media_id))
            .filter(|row| row.status == MediaStatus::Completed)
            .cloned()
            .collect()
    }

    /// The artist collections with at least one downloaded song,
    /// most recently touched first.
    pub fn downloaded_artists(&self) -> Vec<CollectionRow> {
        let mut artists: Vec<&CollectionRow> = self
            .collections
            .values()
            .filter(|c| c.kind == "artist")
            .filter(|c| {
                self.collection_progress(&c.id)
                    .is_some_and(|(completed, _)| completed > 0)
            })
            .collect();
        artists.sort_by_key(|c| std::cmp::Reverse(c.updated_at));
        artists.into_iter().cloned().collect()
    }

    /// Songs queued for download (the resume list at startup).
    pub fn queued_media(&self) -> Vec<MediaRow> {
        self.media
            .values()
            .filter(|row| row.status == MediaStatus::Queued)
            .cloned()
            .collect()
    }

    // ---------- library refs ----------

    fn has_ref(&self, category: &str, target_kind: &str, target_id: &str) -> bool {
        self.refs.iter().any(|r| {
            r.category == category && r.target_kind == target_kind && r.target_id == target_id
        })
    }

    fn add_ref(&mut self, category: &str, target_kind: &str, target_id: &str) {
        let now = now_ms();
        if let Some(existing) = self.refs.iter_mut().find(|r| {
            r.category == category && r.target_kind == target_kind && r.target_id == target_id
        }) {
            existing.updated_at = now;
            return;
        }
        self.refs.push(RefRow {
            category: category.to_string(),
            target_kind: target_kind.to_string(),
            target_id: target_id.to_string(),
            created_at: now,
            updated_at: now,
        });
    }

    fn remove_ref(&mut self, category: &str, target_kind: &str, target_id: &str) -> bool {
        let before = self.refs.len();
        self.refs.retain(|r| {
            !(r.category == category && r.target_kind == target_kind && r.target_id == target_id)
        });
        self.refs.len() != before
    }

    /// Toggle a ref. Returns the new state (true = now present).
    fn toggle_ref(&mut self, category: &str, target_kind: &str, target_id: &str) -> bool {
        if self.remove_ref(category, target_kind, target_id) {
            false
        } else {
            self.add_ref(category, target_kind, target_id);
            true
        }
    }

    /// Refs of one category pointing at media, newest first.
    fn media_refs(&self, category: &str) -> Vec<&RefRow> {
        let mut refs: Vec<&RefRow> = self
            .refs
            .iter()
            .filter(|r| r.category == category && r.target_kind == "media")
            .collect();
        refs.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
        refs
    }

    /// Refs of one category pointing at collections of one kind,
    /// newest first.
    fn collection_refs(&self, category: &str, kind: &str) -> Vec<&RefRow> {
        let mut refs: Vec<&RefRow> = self
            .refs
            .iter()
            .filter(|r| r.category == category && r.target_kind == "collection")
            .filter(|r| {
                self.collections
                    .get(&r.target_id)
                    .is_some_and(|c| c.kind == kind)
            })
            .collect();
        refs.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
        refs
    }

    // ---------- ensuring catalog rows ----------

    /// Make sure a media row exists for this song, refreshing its
    /// metadata. Returns the row's id.
    pub fn ensure_media(&mut self, song: &saavnapi::Song) -> String {
        let now = now_ms();
        match self.media.get_mut(&song.id) {
            Some(row) => row.refresh_from_song(song),
            None => {
                self.media
                    .insert(song.id.clone(), MediaRow::from_song(song, now));
            }
        }
        song.id.clone()
    }

    /// Make sure a collection row exists for this album,
    /// refreshing its metadata.
    pub fn ensure_collection_from_album(&mut self, album: &saavnapi::Album) {
        let now = now_ms();
        let subtitle = artist_line(&album.artists);
        match self.collections.get_mut(&album.id) {
            Some(row) => {
                row.title = album.name.clone();
                row.subtitle = subtitle;
                row.description = album.description.clone();
                row.year = album.year;
                row.language = album.language.clone();
                row.art = best_art(&album.image);
                row.artists = album.artists.clone();
                row.updated_at = now;
            }
            None => {
                self.collections.insert(
                    album.id.clone(),
                    CollectionRow {
                        id: album.id.clone(),
                        kind: "album".to_string(),
                        title: album.name.clone(),
                        subtitle,
                        description: album.description.clone(),
                        year: album.year,
                        language: album.language.clone(),
                        art: best_art(&album.image),
                        cover_id: album.id.clone(),
                        artists: album.artists.clone(),
                        followers: None,
                        created_at: now,
                        updated_at: now,
                    },
                );
            }
        }
    }

    /// Make sure a collection row exists for this playlist.
    pub fn ensure_collection_from_playlist(&mut self, playlist: &saavnapi::Playlist) {
        let now = now_ms();
        match self.collections.get_mut(&playlist.id) {
            Some(row) => {
                row.title = playlist.name.clone();
                row.description = playlist.description.clone();
                row.year = playlist.year;
                row.language = playlist.language.clone();
                row.art = best_art(&playlist.image);
                row.updated_at = now;
            }
            None => {
                self.collections.insert(
                    playlist.id.clone(),
                    CollectionRow {
                        id: playlist.id.clone(),
                        kind: "playlist".to_string(),
                        title: playlist.name.clone(),
                        subtitle: playlist.description.clone().unwrap_or_default(),
                        description: playlist.description.clone(),
                        year: playlist.year,
                        language: playlist.language.clone(),
                        art: best_art(&playlist.image),
                        cover_id: playlist.id.clone(),
                        artists: saavnapi::ArtistGroups::default(),
                        followers: None,
                        created_at: now,
                        updated_at: now,
                    },
                );
            }
        }
    }

    /// Make sure a collection row exists for this artist.
    pub fn ensure_collection_from_artist(&mut self, artist: &saavnapi::Artist) {
        let now = now_ms();
        let subtitle = artist
            .follower_count
            .map(|n| format!("{n} followers"))
            .unwrap_or_default();
        match self.collections.get_mut(&artist.id) {
            Some(row) => {
                row.title = artist.name.clone();
                row.subtitle = subtitle;
                row.art = best_art(&artist.image);
                row.followers = artist.follower_count;
                row.updated_at = now;
            }
            None => {
                self.collections.insert(
                    artist.id.clone(),
                    CollectionRow {
                        id: artist.id.clone(),
                        kind: "artist".to_string(),
                        title: artist.name.clone(),
                        subtitle,
                        description: None,
                        year: None,
                        language: String::new(),
                        art: best_art(&artist.image),
                        cover_id: artist.id.clone(),
                        artists: saavnapi::ArtistGroups::default(),
                        followers: artist.follower_count,
                        created_at: now,
                        updated_at: now,
                    },
                );
            }
        }
    }

    /// Make sure a collection row exists for a details target
    /// (the download / pin path, which only carries the card's
    /// title, subtitle and art). Existing rows are only touched.
    pub fn ensure_collection_stub(
        &mut self,
        kind: &str,
        id: &str,
        title: &str,
        subtitle: &str,
        art: Option<&str>,
    ) {
        let now = now_ms();
        match self.collections.get_mut(id) {
            Some(row) => row.updated_at = now,
            None => {
                self.collections.insert(
                    id.to_string(),
                    CollectionRow {
                        id: id.to_string(),
                        kind: kind.to_string(),
                        title: title.to_string(),
                        subtitle: subtitle.to_string(),
                        description: None,
                        year: None,
                        language: String::new(),
                        art: art.map(str::to_string),
                        cover_id: id.to_string(),
                        artists: saavnapi::ArtistGroups::default(),
                        followers: None,
                        created_at: now,
                        updated_at: now,
                    },
                );
            }
        }
    }

    /// Make sure a media row exists for a pinned song (the pin
    /// path only carries title / artist line / art).
    fn ensure_media_stub(&mut self, id: &str, title: &str, artist: &str, art: Option<&str>) {
        let now = now_ms();
        if !self.media.contains_key(id) {
            self.media.insert(
                id.to_string(),
                MediaRow {
                    id: id.to_string(),
                    title: title.to_string(),
                    artist: artist.to_string(),
                    artists: saavnapi::ArtistGroups {
                        primary: vec![saavnapi::ArtistMap {
                            id: String::new(),
                            name: artist.to_string(),
                            role: String::new(),
                            kind: "artist".to_string(),
                            image: Vec::new(),
                            url: String::new(),
                        }],
                        featured: Vec::new(),
                        all: Vec::new(),
                    },
                    album: None,
                    album_id: None,
                    duration: None,
                    year: None,
                    language: String::new(),
                    has_lyrics: false,
                    lyrics_id: None,
                    art: art.map(str::to_string),
                    cover_id: id.to_string(),
                    status: MediaStatus::NotDownloaded,
                    url: String::new(),
                    total_bytes: None,
                    downloaded_bytes: 0,
                    error: None,
                    created_at: now,
                    updated_at: now,
                },
            );
        }
    }

    // ---------- likes ----------

    pub fn is_liked(&self, song_id: &str) -> bool {
        self.has_ref("liked", "media", song_id)
    }

    /// Toggle a song in liked songs. Returns the new liked state.
    pub fn toggle_like(&mut self, song: &saavnapi::Song) -> bool {
        self.ensure_media(song);
        let state = self.toggle_ref("liked", "media", &song.id);
        self.save();
        state
    }

    pub fn is_album_liked(&self, album_id: &str) -> bool {
        self.has_ref("liked", "collection", album_id)
    }

    /// Toggle an album in liked albums. Returns the new state.
    pub fn toggle_like_album(&mut self, album: &saavnapi::Album) -> bool {
        self.ensure_collection_from_album(album);
        let state = self.toggle_ref("liked", "collection", &album.id);
        self.save();
        state
    }

    pub fn is_playlist_liked(&self, playlist_id: &str) -> bool {
        self.has_ref("liked", "collection", playlist_id)
    }

    /// Toggle a playlist in liked playlists. Returns the new
    /// state.
    pub fn toggle_like_playlist(&mut self, playlist: &saavnapi::Playlist) -> bool {
        self.ensure_collection_from_playlist(playlist);
        let state = self.toggle_ref("liked", "collection", &playlist.id);
        self.save();
        state
    }

    pub fn is_artist_liked(&self, artist_id: &str) -> bool {
        self.has_ref("liked", "collection", artist_id)
    }

    /// Toggle an artist in liked artists. Returns the new state.
    pub fn toggle_like_artist(&mut self, artist: &saavnapi::Artist) -> bool {
        self.ensure_collection_from_artist(artist);
        let state = self.toggle_ref("liked", "collection", &artist.id);
        self.save();
        state
    }

    /// The liked songs, newest first.
    pub fn liked_songs(&self) -> Vec<saavnapi::Song> {
        self.media_refs("liked")
            .into_iter()
            .filter_map(|r| self.media.get(&r.target_id))
            .map(|row| row.to_song(&self.download_dir))
            .collect()
    }

    /// The liked albums, newest first (headers only, no songs).
    pub fn liked_albums(&self) -> Vec<saavnapi::Album> {
        self.collection_refs("liked", "album")
            .into_iter()
            .filter_map(|r| self.collections.get(&r.target_id))
            .map(|row| self.album_for(row, None))
            .collect()
    }

    /// The liked playlists, newest first (headers only).
    pub fn liked_playlists(&self) -> Vec<saavnapi::Playlist> {
        self.collection_refs("liked", "playlist")
            .into_iter()
            .chain(self.collection_refs("liked", "chart"))
            .filter_map(|r| self.collections.get(&r.target_id))
            .map(|row| self.playlist_for(row, None))
            .collect()
    }

    /// The liked artists, newest first (headers only).
    pub fn liked_artists(&self) -> Vec<saavnapi::Artist> {
        self.collection_refs("liked", "artist")
            .into_iter()
            .filter_map(|r| self.collections.get(&r.target_id))
            .map(|row| self.artist_for(row, None))
            .collect()
    }

    // ---------- recents ----------

    /// Record a play: the song's recent ref moves to the front,
    /// and refs past the cap fall off the end.
    pub fn push_recent(&mut self, song: &saavnapi::Song, cap: usize) {
        self.ensure_media(song);
        self.add_ref("recent", "media", &song.id);
        self.trim_recents(cap);
        self.save();
    }

    /// Drop the oldest recent refs past the cap.
    pub fn trim_recents(&mut self, cap: usize) {
        let mut recent: Vec<(String, i64)> = self
            .refs
            .iter()
            .filter(|r| r.category == "recent")
            .map(|r| (r.target_id.clone(), r.updated_at))
            .collect();
        recent.sort_by_key(|(_, updated)| std::cmp::Reverse(*updated));
        let keep: std::collections::HashSet<&str> =
            recent.iter().take(cap).map(|(id, _)| id.as_str()).collect();
        self.refs
            .retain(|r| r.category != "recent" || keep.contains(r.target_id.as_str()));
    }

    /// The recently played songs, most recent first.
    pub fn recent_songs(&self) -> Vec<saavnapi::Song> {
        self.media_refs("recent")
            .into_iter()
            .filter_map(|r| self.media.get(&r.target_id))
            .map(|row| row.to_song(&self.download_dir))
            .collect()
    }

    // ---------- pins ----------

    pub fn is_pinned(&self, kind: &str, id: &str) -> bool {
        let target_kind = if kind == "song" {
            "media"
        } else {
            "collection"
        };
        self.has_ref("pinned", target_kind, id)
    }

    /// Toggle an item's pinned state. Returns the new state.
    pub fn toggle_pin(&mut self, item: &PinnedItem) -> bool {
        if item.kind == "song" {
            self.ensure_media_stub(&item.id, &item.title, &item.subtitle, item.art.as_deref());
            let state = self.toggle_ref("pinned", "media", &item.id);
            self.save();
            state
        } else {
            self.ensure_collection_stub(
                &item.kind,
                &item.id,
                &item.title,
                &item.subtitle,
                item.art.as_deref(),
            );
            let state = self.toggle_ref("pinned", "collection", &item.id);
            self.save();
            state
        }
    }

    /// The pinned items for the sidebar, most recently pinned /
    /// touched first. Art prefers the local cover so it shows
    /// offline.
    pub fn pinned_items(&self) -> Vec<PinnedItem> {
        let mut refs: Vec<&RefRow> = self
            .refs
            .iter()
            .filter(|r| r.category == "pinned")
            .collect();
        refs.sort_by_key(|r| std::cmp::Reverse(r.updated_at));
        refs.into_iter()
            .filter_map(|r| {
                if r.target_kind == "media" {
                    let row = self.media.get(&r.target_id)?;
                    Some(PinnedItem {
                        kind: "song".to_string(),
                        id: row.id.clone(),
                        title: row.title.clone(),
                        subtitle: row.artist.clone(),
                        art: self.cover_uri(&row.cover_id).or_else(|| row.art.clone()),
                    })
                } else {
                    let row = self.collections.get(&r.target_id)?;
                    Some(PinnedItem {
                        kind: row.kind.clone(),
                        id: row.id.clone(),
                        title: row.title.clone(),
                        subtitle: row.subtitle.clone(),
                        art: self.cover_uri(&row.cover_id).or_else(|| row.art.clone()),
                    })
                }
            })
            .collect()
    }

    // ---------- collection reconstruction ----------

    /// Rebuild an `Album` from a collection row, with the given
    /// songs (or none for a header-only use like the Library).
    pub fn album_for(
        &self,
        row: &CollectionRow,
        songs: Option<Vec<saavnapi::Song>>,
    ) -> saavnapi::Album {
        saavnapi::Album {
            id: row.id.clone(),
            name: row.title.clone(),
            description: row.description.clone(),
            year: row.year,
            kind: "album".to_string(),
            play_count: None,
            language: row.language.clone(),
            explicit_content: false,
            artists: row.artists.clone(),
            song_count: Some(
                self.members
                    .get(&row.id)
                    .map(|m| m.len() as u64)
                    .unwrap_or(0),
            ),
            url: String::new(),
            image: row.image_links(&self.download_dir),
            songs,
        }
    }

    /// Rebuild a `Playlist` from a collection row.
    pub fn playlist_for(
        &self,
        row: &CollectionRow,
        songs: Option<Vec<saavnapi::Song>>,
    ) -> saavnapi::Playlist {
        saavnapi::Playlist {
            id: row.id.clone(),
            name: row.title.clone(),
            description: row.description.clone(),
            year: row.year,
            kind: row.kind.clone(),
            play_count: None,
            language: row.language.clone(),
            explicit_content: false,
            song_count: Some(
                self.members
                    .get(&row.id)
                    .map(|m| m.len() as u64)
                    .unwrap_or(0),
            ),
            url: String::new(),
            image: row.image_links(&self.download_dir),
            songs,
            artists: None,
        }
    }

    /// Rebuild an `Artist` from a collection row, with the given
    /// top songs (or none for a header-only use).
    pub fn artist_for(
        &self,
        row: &CollectionRow,
        top_songs: Option<Vec<saavnapi::Song>>,
    ) -> saavnapi::Artist {
        saavnapi::Artist {
            id: row.id.clone(),
            name: row.title.clone(),
            url: String::new(),
            kind: "artist".to_string(),
            image: row.image_links(&self.download_dir),
            follower_count: row.followers,
            fan_count: None,
            is_verified: None,
            dominant_language: None,
            dominant_type: None,
            bio: None,
            dob: None,
            fb: None,
            twitter: None,
            wiki: None,
            available_languages: Vec::new(),
            is_radio_present: None,
            top_songs,
            top_albums: None,
            singles: None,
            similar_artists: None,
        }
    }
}

impl Catalog {
    // ---------- memberships ----------

    /// Add (or touch) a membership of a media in a collection at
    /// a position.
    fn upsert_member(&mut self, collection_id: &str, media_id: &str, position: i64) {
        let now = now_ms();
        let members = self.members.entry(collection_id.to_string()).or_default();
        match members.iter_mut().find(|m| m.media_id == media_id) {
            Some(member) => {
                member.position = position;
                member.updated_at = now;
            }
            None => members.push(MemberRow {
                media_id: media_id.to_string(),
                position,
                created_at: now,
                updated_at: now,
            }),
        }
    }

    fn remove_member(&mut self, collection_id: &str, media_id: &str) {
        if let Some(members) = self.members.get_mut(collection_id) {
            members.retain(|m| m.media_id != media_id);
        }
    }

    /// Does this media belong to any collection (including the
    /// Downloads collection)?
    fn has_memberships(&self, media_id: &str) -> bool {
        self.members
            .values()
            .any(|members| members.iter().any(|m| m.media_id == media_id))
    }

    // ---------- downloads ----------

    /// Prepare a download for a target whose songs have been
    /// fetched: link every song into the collection (or into the
    /// Downloads collection for a lone song), queue the ones that
    /// still need fetching, and return the media rows to spawn
    /// download tasks for. Songs already downloaded are only
    /// touched — never downloaded twice; failed songs retry.
    pub fn prepare_download(
        &mut self,
        target: &TargetInfo,
        songs: &[saavnapi::Song],
        quality: crate::player::Quality,
    ) -> Vec<MediaRow> {
        let now = now_ms();
        let is_collection = target.kind != "song";
        let collection_id = if is_collection {
            self.ensure_collection_stub(
                target.kind,
                target.id,
                target.title,
                target.subtitle,
                target.art,
            );
            target.id.to_string()
        } else {
            DOWNLOADS_ID.to_string()
        };
        if let Some(row) = self.collections.get_mut(&collection_id) {
            row.updated_at = now;
        }
        let mut spawn: Vec<MediaRow> = Vec::new();
        for (index, song) in songs.iter().enumerate() {
            let Some(url) = crate::player::pick_url(song, quality) else {
                continue;
            };
            self.upsert_member(&collection_id, &song.id, index as i64);
            match self.media.get_mut(&song.id) {
                Some(row) => {
                    row.refresh_from_song(song);
                    row.updated_at = now;
                    match row.status {
                        // Already on disk or already in flight:
                        // a touch, nothing to fetch.
                        MediaStatus::Completed | MediaStatus::Queued | MediaStatus::Downloading => {
                        }
                        MediaStatus::Failed | MediaStatus::NotDownloaded => {
                            row.status = MediaStatus::Queued;
                            row.url = url;
                            row.error = None;
                            spawn.push(row.clone());
                        }
                    }
                }
                None => {
                    let mut row = MediaRow::from_song(song, now);
                    row.status = MediaStatus::Queued;
                    row.url = url;
                    self.media.insert(row.id.clone(), row.clone());
                    spawn.push(row);
                }
            }
        }
        self.save();
        spawn
    }

    /// Touch a fully-downloaded collection and all its members —
    /// a re-request that needs no fetch and no downloads.
    pub fn touch_collection(&mut self, collection_id: &str) {
        let now = now_ms();
        if let Some(row) = self.collections.get_mut(collection_id) {
            row.updated_at = now;
        }
        let ids = self.collection_song_ids(collection_id);
        for id in &ids {
            if let Some(row) = self.media.get_mut(id) {
                row.updated_at = now;
            }
        }
        if let Some(members) = self.members.get_mut(collection_id) {
            for member in members.iter_mut() {
                member.updated_at = now;
            }
        }
        self.save();
    }

    /// Record download progress for a song (not persisted per
    /// tick — the `.part` file on disk is the resume truth).
    pub fn mark_progress(&mut self, song_id: &str, downloaded: u64, total: Option<u64>) {
        if let Some(row) = self.media.get_mut(song_id) {
            row.status = MediaStatus::Downloading;
            row.downloaded_bytes = downloaded;
            if total.is_some() {
                row.total_bytes = total;
            }
        }
    }

    /// Mark a song's download complete.
    pub fn mark_completed(&mut self, song_id: &str) {
        if let Some(row) = self.media.get_mut(song_id) {
            row.status = MediaStatus::Completed;
            row.error = None;
            row.updated_at = now_ms();
        }
        self.save();
    }

    /// Mark a song's download failed.
    pub fn mark_failed(&mut self, song_id: &str, error: String) {
        if let Some(row) = self.media.get_mut(song_id) {
            row.status = MediaStatus::Failed;
            row.error = Some(error);
            row.updated_at = now_ms();
        }
        self.save();
    }

    // ---------- deletes ----------

    /// Delete one song's download, wherever it was downloaded
    /// from: the file goes unconditionally, the media row stays
    /// (it may be liked / pinned / a collection member), and its
    /// membership in the Downloads collection goes with the
    /// download. Memberships in real collections stay — the song
    /// remains in their tracklists, just not downloaded.
    pub fn delete_media_download(&mut self, song_id: &str) {
        let _ = std::fs::remove_file(self.music_file(song_id));
        let _ = std::fs::remove_file(part_path(&self.download_dir, song_id));
        if let Some(row) = self.media.get_mut(song_id) {
            row.status = MediaStatus::NotDownloaded;
            row.url = String::new();
            row.total_bytes = None;
            row.downloaded_bytes = 0;
            row.error = None;
            row.updated_at = now_ms();
        }
        self.remove_member(DOWNLOADS_ID, song_id);
        self.save();
    }

    /// Delete a collection's download: its memberships go, and
    /// each member's file is deleted only when that was its last
    /// membership — a song also in another downloaded collection,
    /// or downloaded on its own, keeps its file. Returns the ids
    /// whose files were deleted (so the Shell can drop their
    /// local playback paths). The collection row itself is
    /// dropped when nothing references it any more (not liked,
    /// not pinned); the Downloads collection is never dropped.
    pub fn delete_collection_download(&mut self, collection_id: &str) -> Vec<String> {
        let members = self.members.remove(collection_id).unwrap_or_default();
        let mut deleted_files: Vec<String> = Vec::new();
        for member in &members {
            if self.has_memberships(&member.media_id) {
                continue;
            }
            let deletable = self.media.get(&member.media_id).is_some_and(|row| {
                matches!(row.status, MediaStatus::Completed | MediaStatus::Failed)
            });
            if deletable {
                let _ = std::fs::remove_file(self.music_file(&member.media_id));
                let _ = std::fs::remove_file(part_path(&self.download_dir, &member.media_id));
                if let Some(row) = self.media.get_mut(&member.media_id) {
                    row.status = MediaStatus::NotDownloaded;
                    row.url = String::new();
                    row.total_bytes = None;
                    row.downloaded_bytes = 0;
                    row.error = None;
                    row.updated_at = now_ms();
                }
                deleted_files.push(member.media_id.clone());
            }
        }
        let referenced = self
            .refs
            .iter()
            .any(|r| r.target_kind == "collection" && r.target_id == collection_id);
        if collection_id != DOWNLOADS_ID && !referenced {
            self.collections.remove(collection_id);
        }
        self.save();
        deleted_files
    }

    /// Delete cover files no catalog row references any more.
    fn prune_covers(&self) {
        let referenced: std::collections::HashSet<&str> = self
            .media
            .values()
            .map(|r| r.cover_id.as_str())
            .chain(self.collections.values().map(|r| r.cover_id.as_str()))
            .collect();
        let covers_dir = self.download_dir.join("covers");
        let Ok(entries) = std::fs::read_dir(&covers_dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(id) = name.strip_suffix(".jpg")
                && !referenced.contains(id)
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// Startup sweep: delete audio files whose media is not
    /// completed, stale partials, and unreferenced covers — the
    /// leftovers of interrupted deletes and crashes.
    pub fn sweep_orphans(&self) {
        let music_dir = self.download_dir.join("music");
        if let Ok(entries) = std::fs::read_dir(&music_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(id) = name.strip_suffix(".mp3.part") {
                    let keep = self.media.get(id).is_some_and(|row| {
                        matches!(
                            row.status,
                            MediaStatus::Queued | MediaStatus::Downloading | MediaStatus::Failed
                        )
                    });
                    if !keep {
                        let _ = std::fs::remove_file(entry.path());
                    }
                } else if let Some(id) = name.strip_suffix(".mp3")
                    && !self.is_downloaded(id)
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
        self.prune_covers();
    }
}

// ---------- persistence ----------

/// The SQLite database file under the app folder.
pub fn db_path(app_dir: &std::path::Path) -> PathBuf {
    app_dir.join("resonance.db")
}

/// Open (creating it first) the app's SQLite database and make
/// sure the catalog tables exist. The pre-catalog `collections`
/// table (a different schema) is renamed aside for migration.
pub fn open_db(app_dir: &std::path::Path) -> Result<rusqlite::Connection, rusqlite::Error> {
    let conn = rusqlite::Connection::open(db_path(app_dir))?;
    // The old downloads schema had a `collections` table with a
    // `total_items` column; the catalog's `collections` is a
    // different table. Rename the old one aside, once.
    let old_schema: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table'
             AND name='collections' AND sql LIKE '%total_items%'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n > 0)
        .unwrap_or(false);
    if old_schema {
        let _ = conn.execute("ALTER TABLE collections RENAME TO collections_old", []);
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS media (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            artists TEXT NOT NULL,
            album TEXT,
            album_id TEXT,
            duration INTEGER,
            year TEXT,
            language TEXT NOT NULL,
            has_lyrics INTEGER NOT NULL,
            lyrics_id TEXT,
            art TEXT,
            cover_id TEXT NOT NULL,
            status TEXT NOT NULL,
            url TEXT NOT NULL,
            total_bytes INTEGER,
            downloaded_bytes INTEGER NOT NULL,
            error TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS collections (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            subtitle TEXT NOT NULL,
            description TEXT,
            year INTEGER,
            language TEXT NOT NULL,
            art TEXT,
            cover_id TEXT NOT NULL,
            artists TEXT NOT NULL,
            followers INTEGER,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS collection_media (
            collection_id TEXT NOT NULL,
            media_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (collection_id, media_id)
        );
        CREATE TABLE IF NOT EXISTS library_refs (
            category TEXT NOT NULL,
            target_kind TEXT NOT NULL,
            target_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY (category, target_kind, target_id)
        );",
    )?;
    Ok(conn)
}

impl Catalog {
    /// Persist the whole catalog to SQLite (one transaction;
    /// the data is small, and a full replace keeps deletes
    /// exact).
    pub fn save(&self) {
        let Ok(mut conn) = open_db(&self.app_dir) else {
            return;
        };
        let Ok(tx) = conn.transaction() else {
            return;
        };
        let _ = tx.execute_batch(
            "DELETE FROM media; DELETE FROM collections;
             DELETE FROM collection_media; DELETE FROM library_refs;",
        );
        for row in self.media.values() {
            let _ = tx.execute(
                "INSERT INTO media
                 (id, title, artist, artists, album, album_id, duration,
                  year, language, has_lyrics, lyrics_id, art, cover_id,
                  status, url, total_bytes, downloaded_bytes, error,
                  created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
                         ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
                rusqlite::params![
                    row.id,
                    row.title,
                    row.artist,
                    serde_json::to_string(&row.artists).unwrap_or_default(),
                    row.album,
                    row.album_id,
                    row.duration.map(|n| n as i64),
                    row.year,
                    row.language,
                    row.has_lyrics as i64,
                    row.lyrics_id,
                    row.art,
                    row.cover_id,
                    row.status.as_str(),
                    row.url,
                    row.total_bytes.map(|n| n as i64),
                    row.downloaded_bytes as i64,
                    row.error,
                    row.created_at,
                    row.updated_at,
                ],
            );
        }
        for row in self.collections.values() {
            let _ = tx.execute(
                "INSERT INTO collections
                 (id, kind, title, subtitle, description, year, language,
                  art, cover_id, artists, followers, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                rusqlite::params![
                    row.id,
                    row.kind,
                    row.title,
                    row.subtitle,
                    row.description,
                    row.year,
                    row.language,
                    row.art,
                    row.cover_id,
                    serde_json::to_string(&row.artists).unwrap_or_default(),
                    row.followers.map(|n| n as i64),
                    row.created_at,
                    row.updated_at,
                ],
            );
        }
        for (collection_id, members) in &self.members {
            for member in members {
                let _ = tx.execute(
                    "INSERT INTO collection_media
                     (collection_id, media_id, position, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        collection_id,
                        member.media_id,
                        member.position,
                        member.created_at,
                        member.updated_at,
                    ],
                );
            }
        }
        for r in &self.refs {
            let _ = tx.execute(
                "INSERT INTO library_refs
                 (category, target_kind, target_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    r.category,
                    r.target_kind,
                    r.target_id,
                    r.created_at,
                    r.updated_at,
                ],
            );
        }
        let _ = tx.commit();
    }

    /// Load the catalog from SQLite, migrating the pre-catalog
    /// download tables and `library.json` on first run.
    pub fn load(app_dir: &std::path::Path, download_dir: PathBuf) -> Self {
        let mut catalog = Self {
            media: HashMap::new(),
            collections: HashMap::new(),
            members: HashMap::new(),
            refs: Vec::new(),
            download_dir,
            app_dir: app_dir.to_path_buf(),
        };
        let mut migrated = false;
        if let Ok(conn) = open_db(app_dir) {
            migrated = catalog.migrate_if_needed(&conn);
            catalog.load_media(&conn);
            catalog.load_collections(&conn);
            catalog.load_members(&conn);
            catalog.load_refs(&conn);
        }
        // Anything left mid-download when the app closed goes
        // back to Queued so it resumes on launch. A song's cover
        // is always its own (an earlier build pointed collection
        // members at the collection's cover id).
        for row in catalog.media.values_mut() {
            if row.status == MediaStatus::Downloading {
                row.status = MediaStatus::Queued;
            }
            row.cover_id = row.id.clone();
        }
        catalog.ensure_downloads_collection();
        catalog.sweep_orphans();
        if migrated {
            // Persist the migrated state right away — the old
            // sources are gone, so it must not live only in
            // memory.
            catalog.save();
        }
        catalog
    }

    fn ensure_downloads_collection(&mut self) {
        let now = now_ms();
        self.collections
            .entry(DOWNLOADS_ID.to_string())
            .or_insert_with(|| CollectionRow {
                id: DOWNLOADS_ID.to_string(),
                kind: "downloads".to_string(),
                title: "Downloads".to_string(),
                subtitle: String::new(),
                description: None,
                year: None,
                language: String::new(),
                art: None,
                cover_id: DOWNLOADS_ID.to_string(),
                artists: saavnapi::ArtistGroups::default(),
                followers: None,
                created_at: now,
                updated_at: now,
            });
    }

    fn load_media(&mut self, conn: &rusqlite::Connection) {
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, title, artist, artists, album, album_id, duration,
                    year, language, has_lyrics, lyrics_id, art, cover_id,
                    status, url, total_bytes, downloaded_bytes, error,
                    created_at, updated_at FROM media",
        ) else {
            return;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok(MediaRow {
                id: row.get(0)?,
                title: row.get(1)?,
                artist: row.get(2)?,
                artists: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                album: row.get(4)?,
                album_id: row.get(5)?,
                duration: row.get::<_, Option<i64>>(6)?.map(|n| n as u64),
                year: row.get(7)?,
                language: row.get(8)?,
                has_lyrics: row.get::<_, i64>(9)? != 0,
                lyrics_id: row.get(10)?,
                art: row.get(11)?,
                cover_id: row.get(12)?,
                status: MediaStatus::from_str(&row.get::<_, String>(13)?),
                url: row.get(14)?,
                total_bytes: row.get::<_, Option<i64>>(15)?.map(|n| n as u64),
                downloaded_bytes: row.get::<_, i64>(16)? as u64,
                error: row.get(17)?,
                created_at: row.get(18)?,
                updated_at: row.get(19)?,
            })
        }) else {
            return;
        };
        for row in rows.flatten() {
            self.media.insert(row.id.clone(), row);
        }
    }

    fn load_collections(&mut self, conn: &rusqlite::Connection) {
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, kind, title, subtitle, description, year, language,
                    art, cover_id, artists, followers, created_at, updated_at
             FROM collections",
        ) else {
            return;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok(CollectionRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                subtitle: row.get(3)?,
                description: row.get(4)?,
                year: row.get(5)?,
                language: row.get(6)?,
                art: row.get(7)?,
                cover_id: row.get(8)?,
                artists: serde_json::from_str(&row.get::<_, String>(9)?).unwrap_or_default(),
                followers: row.get::<_, Option<i64>>(10)?.map(|n| n as u64),
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        }) else {
            return;
        };
        for row in rows.flatten() {
            self.collections.insert(row.id.clone(), row);
        }
    }

    fn load_members(&mut self, conn: &rusqlite::Connection) {
        let Ok(mut stmt) = conn.prepare(
            "SELECT collection_id, media_id, position, created_at, updated_at
             FROM collection_media",
        ) else {
            return;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                MemberRow {
                    media_id: row.get(1)?,
                    position: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                },
            ))
        }) else {
            return;
        };
        for (collection_id, member) in rows.flatten() {
            self.members.entry(collection_id).or_default().push(member);
        }
    }

    fn load_refs(&mut self, conn: &rusqlite::Connection) {
        let Ok(mut stmt) = conn.prepare(
            "SELECT category, target_kind, target_id, created_at, updated_at
             FROM library_refs",
        ) else {
            return;
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok(RefRow {
                category: row.get(0)?,
                target_kind: row.get(1)?,
                target_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        }) else {
            return;
        };
        self.refs = rows.flatten().collect();
    }
}

// ---------- migration (pre-catalog downloads + library.json) ----------

/// A row of the old `downloads` table.
struct OldItem {
    song_id: String,
    title: String,
    artist: String,
    collection_id: Option<String>,
    url: String,
    total_bytes: Option<u64>,
    downloaded_bytes: u64,
    status: String,
    error: Option<String>,
    file_name: String,
    subfolder: String,
    art: Option<String>,
    album: Option<String>,
    duration: Option<u64>,
}

#[derive(Deserialize)]
struct OldLibrary {
    #[serde(default)]
    liked_songs: Vec<saavnapi::Song>,
    #[serde(default)]
    liked_albums: Vec<saavnapi::Album>,
    #[serde(default)]
    liked_playlists: Vec<saavnapi::Playlist>,
    #[serde(default)]
    liked_artists: Vec<saavnapi::Artist>,
    #[serde(default)]
    recent_songs: Vec<saavnapi::Song>,
    #[serde(default)]
    pinned: Vec<OldPinned>,
}

#[derive(Deserialize)]
struct OldPinned {
    kind: String,
    id: String,
    title: String,
    subtitle: String,
    art: Option<String>,
}

impl Catalog {
    /// One-time migration: the old download tables become media /
    /// collections / memberships (audio files move into
    /// `music/<id>.mp3`; covers already live in `covers/`), and
    /// `library.json` becomes catalog rows + refs (the file is
    /// renamed aside, not deleted). Returns true when anything
    /// was migrated, so the caller can persist the new state.
    fn migrate_if_needed(&mut self, conn: &rusqlite::Connection) -> bool {
        let media_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM media", [], |row| row.get(0))
            .unwrap_or(0);
        if media_count > 0 {
            return false;
        }
        let has_old_downloads: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='downloads'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);
        let mut migrated = false;
        if has_old_downloads {
            self.migrate_old_downloads(conn);
            let _ = conn.execute_batch(
                "DROP TABLE IF EXISTS downloads;
                 DROP TABLE IF EXISTS collection_songs;
                 DROP TABLE IF EXISTS collections_old;",
            );
            migrated = true;
        }
        if self.migrate_library_json() {
            migrated = true;
        }
        migrated
    }

    fn migrate_old_downloads(&mut self, conn: &rusqlite::Connection) {
        let now = now_ms();
        // Old collections (renamed aside by open_db).
        let old_collections: Vec<(String, String, String, Option<String>)> = conn
            .prepare("SELECT id, kind, title, art FROM collections_old")
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                })
                .ok()
                .map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();
        for (id, kind, title, art) in old_collections {
            self.collections.insert(
                id.clone(),
                CollectionRow {
                    id: id.clone(),
                    kind,
                    title,
                    subtitle: String::new(),
                    description: None,
                    year: None,
                    language: String::new(),
                    art,
                    cover_id: id,
                    artists: saavnapi::ArtistGroups::default(),
                    followers: None,
                    created_at: now,
                    updated_at: now,
                },
            );
        }
        // Old items -> media rows (+ file moves).
        let items: Vec<OldItem> = conn
            .prepare(
                "SELECT song_id, title, artist, collection_id, url, total_bytes,
                        downloaded_bytes, status, error, file_name, subfolder,
                        art, album, duration FROM downloads",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok(OldItem {
                        song_id: row.get(0)?,
                        title: row.get(1)?,
                        artist: row.get(2)?,
                        collection_id: row.get(3)?,
                        url: row.get(4)?,
                        total_bytes: row.get::<_, Option<i64>>(5)?.map(|n| n as u64),
                        downloaded_bytes: row.get::<_, i64>(6)? as u64,
                        status: row.get(7)?,
                        error: row.get(8)?,
                        file_name: row.get(9)?,
                        subfolder: row.get(10)?,
                        art: row.get(11)?,
                        album: row.get(12)?,
                        duration: row.get::<_, Option<i64>>(13)?.map(|n| n as u64),
                    })
                })
                .ok()
                .map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();
        let music_dir = self.download_dir.join("music");
        let _ = std::fs::create_dir_all(&music_dir);
        for item in &items {
            let mut status = MediaStatus::from_str(&item.status);
            // Move the audio file into music/<id>.mp3.
            let mut old_path = self.download_dir.clone();
            if !item.subfolder.is_empty() {
                old_path.push(&item.subfolder);
            }
            old_path.push(&item.file_name);
            let new_path = music_path(&self.download_dir, &item.song_id);
            if old_path.exists() {
                let _ = std::fs::rename(&old_path, &new_path);
            }
            let old_part = {
                let mut p = old_path.clone();
                let name = format!(
                    "{}.part",
                    p.file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                );
                p.set_file_name(name);
                p
            };
            if old_part.exists() {
                let _ = std::fs::rename(&old_part, part_path(&self.download_dir, &item.song_id));
            }
            if status == MediaStatus::Completed && !new_path.exists() {
                // Marked downloaded but the file is gone.
                status = MediaStatus::NotDownloaded;
            }
            self.media.insert(
                item.song_id.clone(),
                MediaRow {
                    id: item.song_id.clone(),
                    title: item.title.clone(),
                    artist: item.artist.clone(),
                    artists: saavnapi::ArtistGroups {
                        primary: vec![saavnapi::ArtistMap {
                            id: String::new(),
                            name: item.artist.clone(),
                            role: String::new(),
                            kind: "artist".to_string(),
                            image: Vec::new(),
                            url: String::new(),
                        }],
                        featured: Vec::new(),
                        all: Vec::new(),
                    },
                    album: item.album.clone(),
                    album_id: None,
                    duration: item.duration,
                    year: None,
                    language: String::new(),
                    has_lyrics: false,
                    lyrics_id: None,
                    art: item.art.clone(),
                    cover_id: item.song_id.clone(),
                    status,
                    url: item.url.clone(),
                    total_bytes: item.total_bytes,
                    downloaded_bytes: item.downloaded_bytes,
                    error: item.error.clone(),
                    created_at: now,
                    updated_at: now,
                },
            );
            // Membership: the Downloads collection for lone
            // songs; appended to the collection's list otherwise
            // (the recorded track order below may refine it —
            // the upsert dedupes).
            let collection_id = item
                .collection_id
                .clone()
                .unwrap_or_else(|| DOWNLOADS_ID.to_string());
            let position = self
                .members
                .get(&collection_id)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            self.upsert_member(&collection_id, &item.song_id, position);
        }
        // The recorded track order, where it exists.
        let recorded: Vec<(String, String, i64)> = conn
            .prepare(
                "SELECT collection_id, song_id, position FROM collection_songs
                 ORDER BY position",
            )
            .ok()
            .and_then(|mut stmt| {
                stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                })
                .ok()
                .map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();
        for (collection_id, song_id, position) in recorded {
            self.upsert_member(&collection_id, &song_id, position);
        }
    }

    /// Migrate `library.json` into catalog rows + refs. The file
    /// is renamed to `library.json.migrated` afterwards.
    fn migrate_library_json(&mut self) -> bool {
        let path = self.app_dir.join("library.json");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return false;
        };
        let Ok(old) = serde_json::from_str::<OldLibrary>(&text) else {
            return false;
        };
        let now = now_ms();
        for song in &old.liked_songs {
            self.ensure_media(song);
            self.refs.push(RefRow {
                category: "liked".to_string(),
                target_kind: "media".to_string(),
                target_id: song.id.clone(),
                created_at: now,
                updated_at: now,
            });
        }
        for album in &old.liked_albums {
            self.ensure_collection_from_album(album);
            self.refs.push(RefRow {
                category: "liked".to_string(),
                target_kind: "collection".to_string(),
                target_id: album.id.clone(),
                created_at: now,
                updated_at: now,
            });
        }
        for playlist in &old.liked_playlists {
            self.ensure_collection_from_playlist(playlist);
            self.refs.push(RefRow {
                category: "liked".to_string(),
                target_kind: "collection".to_string(),
                target_id: playlist.id.clone(),
                created_at: now,
                updated_at: now,
            });
        }
        for artist in &old.liked_artists {
            self.ensure_collection_from_artist(artist);
            self.refs.push(RefRow {
                category: "liked".to_string(),
                target_kind: "collection".to_string(),
                target_id: artist.id.clone(),
                created_at: now,
                updated_at: now,
            });
        }
        // Recents: index 0 is the most recent — stagger the
        // timestamps so the order survives.
        for (index, song) in old.recent_songs.iter().enumerate() {
            self.ensure_media(song);
            let stamp = now - index as i64;
            self.refs.push(RefRow {
                category: "recent".to_string(),
                target_kind: "media".to_string(),
                target_id: song.id.clone(),
                created_at: stamp,
                updated_at: stamp,
            });
        }
        // Pinned: the old list is in pin order (first pinned
        // first); the sidebar shows most recent first, so the
        // last pinned gets the largest timestamp.
        for (index, item) in old.pinned.iter().enumerate() {
            let stamp = now + index as i64;
            if item.kind == "song" {
                self.ensure_media_stub(&item.id, &item.title, &item.subtitle, item.art.as_deref());
                self.refs.push(RefRow {
                    category: "pinned".to_string(),
                    target_kind: "media".to_string(),
                    target_id: item.id.clone(),
                    created_at: stamp,
                    updated_at: stamp,
                });
            } else {
                self.ensure_collection_stub(
                    &item.kind,
                    &item.id,
                    &item.title,
                    &item.subtitle,
                    item.art.as_deref(),
                );
                self.refs.push(RefRow {
                    category: "pinned".to_string(),
                    target_kind: "collection".to_string(),
                    target_id: item.id.clone(),
                    created_at: stamp,
                    updated_at: stamp,
                });
            }
        }
        let _ = std::fs::rename(&path, self.app_dir.join("library.json.migrated"));
        true
    }
}

// ---------- files ----------

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

// ---------- the download engine ----------

/// Fetch a cover image into its file. Best-effort: art never
/// fails a download.
async fn fetch_cover(url: &str, path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let Ok(response) = reqwest::Client::new().get(url).send().await else {
        return;
    };
    if !response.status().is_success() {
        return;
    }
    if let Ok(bytes) = response.bytes().await {
        let _ = tokio::fs::write(path, &bytes).await;
    }
}

/// Download a song's own cover into `covers/<cover_id>.jpg`,
/// unless it is already there.
async fn cache_cover(row: &MediaRow, download_dir: &std::path::Path) {
    let Some(url) = row.art.as_ref() else {
        return;
    };
    let path = cover_path(download_dir, &row.cover_id);
    if path.exists() {
        return;
    }
    fetch_cover(url, &path).await;
}

/// Download a collection's OWN cover (the album / playlist /
/// artist art) into `covers/<collection_id>.jpg`. Called when
/// a collection download is requested; it overwrites, so the
/// file always holds the collection's current art.
pub async fn cache_collection_cover(download_dir: &std::path::Path, cover_id: &str, url: &str) {
    let path = cover_path(download_dir, cover_id);
    fetch_cover(url, &path).await;
}

/// Download one song to `music/<id>.mp3`, resuming a partial
/// file and retrying on network failure. Sends progress /
/// completion / failure events to the Shell.
pub async fn download_media(
    row: MediaRow,
    download_dir: PathBuf,
    events: mpsc::Sender<DownloadEvent>,
) -> Result<(), String> {
    let final_path = music_path(&download_dir, &row.id);
    let part_file = part_path(&download_dir, &row.id);
    if let Some(parent) = final_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }

    let client = reqwest::Client::new();
    let mut downloaded: u64 = 0;
    // Resume from a partial file when one exists.
    if let Ok(meta) = tokio::fs::metadata(&part_file).await {
        downloaded = meta.len();
    }
    let mut total: Option<u64> = row.total_bytes;
    let mut last_progress = tokio::time::Instant::now();

    for attempt in 1..=MAX_ATTEMPTS {
        let mut request = client.get(&row.url);
        if downloaded > 0 {
            request = request.header("Range", format!("bytes={downloaded}-"));
        }
        let mut response = match request.send().await {
            Ok(response) => response,
            Err(err) => {
                if attempt == MAX_ATTEMPTS {
                    let _ = events
                        .send(DownloadEvent::Failed {
                            song_id: row.id.clone(),
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
            let _ = tokio::fs::remove_file(&part_file).await;
        }
        if total.is_none() {
            total = response.content_length().map(|len| len + downloaded);
        }
        if !status.is_success() {
            if attempt == MAX_ATTEMPTS {
                let message = format!("Server error: {status}");
                let _ = events
                    .send(DownloadEvent::Failed {
                        song_id: row.id.clone(),
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
            .open(&part_file)
            .await
        {
            Ok(file) => file,
            Err(err) => {
                let message = format!("Cannot write file: {err}");
                let _ = events
                    .send(DownloadEvent::Failed {
                        song_id: row.id.clone(),
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
                                song_id: row.id.clone(),
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
                        song_id: row.id.clone(),
                        error: err.clone(),
                    })
                    .await;
                return Err(err);
            }
            tokio::time::sleep(RETRY_BASE_DELAY * attempt).await;
            continue;
        }

        // Done — move the partial file into place.
        if tokio::fs::rename(&part_file, &final_path).await.is_err() {
            // Cross-device rename can fail; copy instead.
            if tokio::fs::copy(&part_file, &final_path).await.is_err() {
                let message = "Cannot save file".to_string();
                let _ = events
                    .send(DownloadEvent::Failed {
                        song_id: row.id.clone(),
                        error: message.clone(),
                    })
                    .await;
                return Err(message);
            }
            let _ = tokio::fs::remove_file(&part_file).await;
        }
        // Cache the cover alongside, so the art shows offline.
        cache_cover(&row, &download_dir).await;
        let _ = events
            .send(DownloadEvent::Progress {
                song_id: row.id.clone(),
                downloaded,
                total,
            })
            .await;
        let _ = events
            .send(DownloadEvent::Completed {
                song_id: row.id.clone(),
            })
            .await;
        return Ok(());
    }
    Err("Download failed".to_string())
}
