//! Persisted settings, as JSON under `%LOCALAPPDATA%\Resonance`.
//! All loads are best-effort — a missing or corrupt file yields
//! defaults, never an error. (The library — liked / pinned /
//! recent / downloads — lives in the SQLite catalog instead;
//! see `catalog.rs`.)

use std::fs;
use std::path::PathBuf;

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
    /// Download a song / album / playlist automatically when it is
    /// liked. On by default.
    #[serde(default = "default_true")]
    pub auto_download_liked: bool,
    /// Download an item automatically when it is pinned to the
    /// sidebar. On by default.
    #[serde(default = "default_true")]
    pub auto_download_pinned: bool,
    /// Download a song automatically when it is played (it joins
    /// the recent list). On by default.
    #[serde(default = "default_true")]
    pub auto_download_recent: bool,
    /// How much disk the image cache may use, in megabytes.
    /// The cache evicts the least-recently-used images past it.
    #[serde(default = "default_image_cache_limit_mb")]
    pub image_cache_limit_mb: u64,
    /// The quality downloaded files are saved at (separate from
    /// the stream quality — a download can be higher quality than
    /// the stream, or lower to save space).
    #[serde(default)]
    pub download_quality: QualitySetting,
    /// How many recently played songs are kept. Customizable in
    /// Settings; 100 by default.
    #[serde(default = "default_recent_max")]
    pub recent_max: u64,
}

fn default_recent_max() -> u64 {
    100
}

fn default_true() -> bool {
    true
}

fn default_image_cache_limit_mb() -> u64 {
    500
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
            auto_download_liked: true,
            auto_download_pinned: true,
            auto_download_recent: true,
            image_cache_limit_mb: 500,
            download_quality: QualitySetting::High,
            recent_max: 100,
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
/// render the row and open/play it; the catalog builds these from
/// its rows for the pinned refs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedItem {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub art: Option<String>,
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
