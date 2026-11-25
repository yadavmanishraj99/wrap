//! Audio playback via WinRT `MediaPlayer` (`windows` crate).
//!
//! The player owns the queue; the Shell owns the player and mirrors
//! its state (current song, position, duration, playing) into the UI.
//! All WinRT calls are fallible — a missing `MediaPlayer` (no audio
//! device, headless test run) degrades to a no-op player instead of
//! crashing the app.

use saavnapi::Song;
use windows::Foundation::{TimeSpan, Uri};
use windows::Media::Core::MediaSource;
use windows::Media::Playback::{MediaPlaybackState, MediaPlayer};
use windows::core::HSTRING;

/// Stream quality, mapped onto saavnapi's `download_url` qualities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

impl Quality {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "96kbps",
            Self::Medium => "160kbps",
            Self::High => "320kbps",
        }
    }

    pub fn from_setting(setting: crate::store::QualitySetting) -> Self {
        match setting {
            crate::store::QualitySetting::Low => Self::Low,
            crate::store::QualitySetting::Medium => Self::Medium,
            crate::store::QualitySetting::High => Self::High,
        }
    }
}

/// Pick the stream URL for a song at the requested quality, falling
/// back to the best available variant.
pub fn pick_url(song: &Song, quality: Quality) -> Option<String> {
    let want = quality.label();
    song.download_url
        .iter()
        .find(|link| link.quality == want)
        .or_else(|| song.download_url.last())
        .map(|link| link.url.clone())
}

pub struct Player {
    inner: Option<MediaPlayer>,
    pub queue: Vec<Song>,
    pub index: Option<usize>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            inner: MediaPlayer::new().ok(),
            queue: Vec::new(),
            index: None,
        }
    }

    pub fn current(&self) -> Option<&Song> {
        self.index.and_then(|i| self.queue.get(i))
    }

    pub fn has_current(&self) -> bool {
        self.current().is_some()
    }

    /// Replace the queue and start playing `index`.
    pub fn load_queue(&mut self, queue: Vec<Song>, index: usize, quality: Quality) {
        self.queue = queue;
        self.index = Some(index);
        let _ = self.play_current(quality);
    }

    /// (Re)start playback of the current queue item.
    pub fn play_current(&self, quality: Quality) -> windows::core::Result<()> {
        let Some(player) = &self.inner else {
            return Ok(());
        };
        let Some(song) = self.current() else {
            return Ok(());
        };
        let Some(url) = pick_url(song, quality) else {
            return Ok(());
        };
        let uri = Uri::CreateUri(&HSTRING::from(url))?;
        let source = MediaSource::CreateFromUri(&uri)?;
        player.SetSource(&source)?;
        player.Play()?;
        Ok(())
    }

    pub fn play(&self) -> windows::core::Result<()> {
        if let Some(player) = &self.inner {
            player.Play()?;
        }
        Ok(())
    }

    pub fn pause(&self) -> windows::core::Result<()> {
        if let Some(player) = &self.inner {
            player.Pause()?;
        }
        Ok(())
    }

    /// Toggle play/pause. Returns the new playing state.
    pub fn toggle(&self) -> bool {
        if self.is_playing() {
            let _ = self.pause();
            false
        } else {
            let _ = self.play();
            true
        }
    }

    pub fn is_playing(&self) -> bool {
        self.inner
            .as_ref()
            .and_then(|p| p.PlaybackSession().ok())
            .and_then(|s| s.PlaybackState().ok())
            .map(|state| state == MediaPlaybackState::Playing)
            .unwrap_or(false)
    }

    pub fn position_secs(&self) -> f64 {
        self.inner
            .as_ref()
            .and_then(|p| p.PlaybackSession().ok())
            .and_then(|s| s.Position().ok())
            .map(|t| t.Duration as f64 / 10_000_000.0)
            .unwrap_or(0.0)
    }

    pub fn duration_secs(&self) -> f64 {
        self.inner
            .as_ref()
            .and_then(|p| p.NaturalDuration().ok())
            .map(|t| t.Duration as f64 / 10_000_000.0)
            .unwrap_or(0.0)
    }

    pub fn seek(&self, secs: f64) {
        if let Some(player) = &self.inner
            && let Ok(session) = player.PlaybackSession()
        {
            let _ = session.SetPosition(TimeSpan {
                Duration: (secs * 10_000_000.0) as i64,
            });
        }
    }

    pub fn set_volume(&self, volume: f64) {
        if let Some(player) = &self.inner {
            let _ = player.SetVolume(volume.clamp(0.0, 1.0));
        }
    }

    pub fn next(&mut self, quality: Quality) {
        if let Some(i) = self.index
            && i + 1 < self.queue.len()
        {
            self.index = Some(i + 1);
            let _ = self.play_current(quality);
        }
    }

    /// Jump to `index` and start playing it (no-op out of range).
    /// Used by repeat-all to wrap to the start of the queue.
    pub fn play_index(&mut self, index: usize, quality: Quality) {
        if index < self.queue.len() {
            self.index = Some(index);
            let _ = self.play_current(quality);
        }
    }

    /// Restart the current song from the beginning (repeat-one).
    pub fn restart(&self) {
        self.seek(0.0);
        let _ = self.play();
    }

    /// True when the current song is the last one in the queue.
    pub fn is_last(&self) -> bool {
        match self.index {
            Some(i) => !self.queue.is_empty() && i + 1 >= self.queue.len(),
            None => false,
        }
    }

    /// Insert songs right after the current one (Play next). Songs
    /// already anywhere in the queue are skipped. With no current
    /// song this does nothing — the caller loads the queue instead.
    pub fn insert_next(&mut self, songs: Vec<Song>) {
        let Some(i) = self.index else {
            return;
        };
        let at = i + 1;
        let fresh: Vec<Song> = songs
            .into_iter()
            .filter(|song| !self.queue.iter().any(|queued| queued.id == song.id))
            .collect();
        for (offset, song) in fresh.into_iter().enumerate() {
            self.queue.insert(at + offset, song);
        }
    }

    /// Shuffle the songs after the current one, keeping the song
    /// that's playing (and everything before it) in place. A tiny
    /// xorshift RNG keeps this dependency-free.
    pub fn shuffle_upcoming(&mut self) {
        let Some(i) = self.index else {
            return;
        };
        let start = i + 1;
        if start >= self.queue.len() {
            return;
        }
        let mut state = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1;
        let mut n = self.queue.len() - start;
        while n > 1 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let j = (state % n as u64) as usize;
            n -= 1;
            self.queue.swap(start + n, start + j);
        }
    }

    pub fn previous(&mut self, quality: Quality) {
        if let Some(i) = self.index
            && i > 0
        {
            self.index = Some(i - 1);
            let _ = self.play_current(quality);
        }
    }
}
