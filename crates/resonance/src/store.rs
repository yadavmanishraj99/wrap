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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Library {
    pub liked_songs: Vec<Song>,
    pub liked_albums: Vec<Album>,
    pub liked_playlists: Vec<Playlist>,
    pub recent_songs: Vec<Song>,
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

    /// Record a play at the front of recents (deduplicated, capped).
    pub fn push_recent(&mut self, song: Song) {
        self.recent_songs.retain(|s| s.id != song.id);
        self.recent_songs.insert(0, song);
        self.recent_songs.truncate(50);
        self.save();
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
