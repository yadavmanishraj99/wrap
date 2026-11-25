//! Persisted state: the library (liked songs/albums/playlists,
//! recents) and settings, as JSON under `%LOCALAPPDATA%\Resonance`.
//! All loads are best-effort — a missing or corrupt file yields
//! defaults, never an error.

use std::fs;
use std::path::PathBuf;

use saavnapi::{Album, Playlist, Song};
use serde::{Deserialize, Serialize};

pub fn app_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Resonance")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Default for Theme {
    /// Resonance's design system is dark-first.
    fn default() -> Self {
        Self::Dark
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QualitySetting {
    Low,
    Medium,
    #[default]
    High,
}

impl QualitySetting {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "96 kbps",
            Self::Medium => "160 kbps",
            Self::High => "320 kbps",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    /// Cycle off -> all -> one -> off (the repeat button's order).
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }
}

/// Window backdrop material (WinUI `WindowBackdrop`). Mica is the
/// default — the subtle desktop-tinted material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Material {
    Mica,
    MicaAlt,
    #[default]
    Acrylic,
    None,
}

impl Material {
    pub fn label(self) -> &'static str {
        match self {
            Self::Mica => "Mica",
            Self::MicaAlt => "Mica Alt",
            Self::Acrylic => "Acrylic",
            Self::None => "Solid (no material)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: Theme,
    pub quality: QualitySetting,
    pub volume: f64,
    pub autoplay: bool,
    /// Shuffle the upcoming queue when enabled.
    #[serde(default)]
    pub shuffle: bool,
    /// Repeat mode for the end of the queue.
    #[serde(default)]
    pub repeat: RepeatMode,
    /// Window backdrop material (Acrylic by default — frosted glass
    /// for the Now Playing sidebar, which is translucent over the
    /// window. Acrylic is supported on every OS that can run WinUI 3
    /// — Windows 10 1809 and later — so it is the default; Mica Alt
    /// is the fallback, pickable in Settings).
    #[serde(default)]
    pub material: Material,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            quality: QualitySetting::High,
            volume: 0.8,
            autoplay: true,
            shuffle: false,
            repeat: RepeatMode::Off,
            material: Material::Acrylic,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        load_json("settings.json").unwrap_or_default()
    }

    pub fn save(&self) {
        save_json("settings.json", self);
    }
}

/// A pinned media item (song, artist, album, playlist, or chart) —
/// shown in the sidebar below the Library divider. Just enough to
/// render the row and open/play it; the full object is fetched on
/// demand (a pinned song is fetched by id when clicked to play).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedItem {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub art: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Library {
    pub liked_songs: Vec<Song>,
    pub liked_albums: Vec<Album>,
    pub liked_playlists: Vec<Playlist>,
    pub recent_songs: Vec<Song>,
    /// Pinned media items, in pin order (first pinned at the top of
    /// the sidebar section). `#[serde(default)]` so a library.json
    /// written before pinning existed still loads.
    #[serde(default)]
    pub pinned: Vec<PinnedItem>,
}

impl Library {
    pub fn load() -> Self {
        load_json("library.json").unwrap_or_default()
    }

    pub fn save(&self) {
        save_json("library.json", self);
    }

    pub fn is_liked(&self, song_id: &str) -> bool {
        self.liked_songs.iter().any(|song| song.id == song_id)
    }

    /// Toggle a song in liked songs. Returns the new liked state.
    pub fn toggle_like(&mut self, song: Song) -> bool {
        if let Some(pos) = self.liked_songs.iter().position(|s| s.id == song.id) {
            self.liked_songs.remove(pos);
            self.save();
            false
        } else {
            self.liked_songs.push(song);
            self.save();
            true
        }
    }

    /// Is this album liked?
    pub fn is_album_liked(&self, album_id: &str) -> bool {
        self.liked_albums.iter().any(|a| a.id == album_id)
    }

    /// Toggle an album in liked albums. Returns the new liked state.
    pub fn toggle_like_album(&mut self, album: Album) -> bool {
        if let Some(pos) = self.liked_albums.iter().position(|a| a.id == album.id) {
            self.liked_albums.remove(pos);
            self.save();
            false
        } else {
            self.liked_albums.push(album);
            self.save();
            true
        }
    }

    /// Is this playlist liked?
    pub fn is_playlist_liked(&self, playlist_id: &str) -> bool {
        self.liked_playlists.iter().any(|p| p.id == playlist_id)
    }

    /// Toggle a playlist in liked playlists. Returns the new liked state.
    pub fn toggle_like_playlist(&mut self, playlist: Playlist) -> bool {
        if let Some(pos) = self
            .liked_playlists
            .iter()
            .position(|p| p.id == playlist.id)
        {
            self.liked_playlists.remove(pos);
            self.save();
            false
        } else {
            self.liked_playlists.push(playlist);
            self.save();
            true
        }
    }

    /// Record a play at the front of recents (deduplicated, capped).
    pub fn push_recent(&mut self, song: Song) {
        self.recent_songs.retain(|s| s.id != song.id);
        self.recent_songs.insert(0, song);
        self.recent_songs.truncate(50);
        self.save();
    }

    /// Is this item pinned?
    pub fn is_pinned(&self, kind: &str, id: &str) -> bool {
        self.pinned.iter().any(|p| p.kind == kind && p.id == id)
    }

    /// Toggle an item's pinned state. Returns the new pinned state.
    pub fn toggle_pin(&mut self, item: PinnedItem) -> bool {
        if let Some(pos) = self
            .pinned
            .iter()
            .position(|p| p.kind == item.kind && p.id == item.id)
        {
            self.pinned.remove(pos);
            self.save();
            false
        } else {
            self.pinned.push(item);
            self.save();
            true
        }
    }
}

fn load_json<T>(name: &str) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    let text = fs::read_to_string(app_dir().join(name)).ok()?;
    serde_json::from_str(&text).ok()
}

fn save_json<T>(name: &str, value: &T)
where
    T: Serialize,
{
    let dir = app_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(text) = serde_json::to_string_pretty(value) {
        let _ = fs::write(dir.join(name), text);
    }
}
