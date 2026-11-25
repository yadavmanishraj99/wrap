//! Audio playback via WinRT `MediaPlayer` (`windows` crate).
//!
//! The player owns the queue; the Shell owns the player and mirrors
//! its state (current song, position, duration, playing) into the UI.
//! All WinRT calls are fallible — a missing `MediaPlayer` (no audio
//! device, headless test run) degrades to a no-op player instead of
//! crashing the app.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};

use saavnapi::Song;
use windows::Foundation::{TimeSpan, TypedEventHandler, Uri};
use windows::Media::Core::MediaSource;
use windows::Media::Playback::{MediaPlaybackState, MediaPlayer};
use windows::Media::{
    MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
    SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    SystemMediaTransportControlsTimelineProperties,
};
use windows::Storage::Streams::RandomAccessStreamReference;
use windows::core::HSTRING;

/// The Windows app identity this process runs under. Set as the
/// process AppUserModelID at startup (main.rs) and as the media
/// session's AppMediaId, so the session (and the taskbar) shows
/// "Resonance" instead of "Unknown app". The name fully resolves
/// once the app is installed/packaged with a matching registered
/// shortcut; for a `cargo run` build this is best-effort.
pub const APP_USER_MODEL_ID: &str = "YadavManishraj.Resonance";

/// A button press from the OS media session (SMTC), delivered to
/// the Shell over a channel — the WinRT event handler runs on a
/// system thread, so it cannot send a Reactor message directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtcCommand {
    Play,
    Pause,
    Next,
    Previous,
}

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

/// The primary artist names, comma-joined, for the media session's
/// artist field (same shape as the UI's artist line).
fn artist_names(song: &Song) -> String {
    if song.artists.primary.is_empty() {
        "Unknown artist".to_string()
    } else {
        song.artists
            .primary
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub struct Player {
    inner: Option<MediaPlayer>,
    /// The OS media session we own (direct SMTC, see setup_smtc).
    smtc: Option<SystemMediaTransportControls>,
    /// The receiving end of the SMTC button channel; the Shell
    /// takes it once and listens for button presses on it.
    smtc_rx: Option<Receiver<SmtcCommand>>,
    /// Whether repeat-all is on (set by the Shell, which owns the
    /// setting). Decides whether Next stays enabled on the last
    /// song — with repeat-all it wraps to the start.
    repeat_all: bool,
    /// The currently loaded media source, kept so it can be closed
    /// explicitly when the next track starts. `MediaSource::Close`
    /// releases its stream and network resources at once, instead
    /// of leaving them to COM refcounting when the player moves on.
    current_source: Option<MediaSource>,
    /// Downloaded songs by id, mapped to their local file path.
    /// When a song is in here, `play_current` plays the local file
    /// instead of streaming — so a downloaded song plays offline.
    /// The Shell keeps this in sync with the download queue.
    local_paths: HashMap<String, PathBuf>,
    pub queue: Vec<Song>,
    pub index: Option<usize>,
}

impl Player {
    pub fn new() -> Self {
        let inner = MediaPlayer::new().ok();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut player = Self {
            inner,
            smtc: None,
            smtc_rx: Some(rx),
            repeat_all: false,
            current_source: None,
            local_paths: HashMap::new(),
            queue: Vec::new(),
            index: None,
        };
        player.setup_smtc(tx);
        player
    }

    /// Record a downloaded song's local file, so it plays from disk
    /// instead of streaming. Called by the Shell whenever a download
    /// completes (and on startup, for the persisted queue).
    pub fn set_local_path(&mut self, song_id: String, path: PathBuf) {
        self.local_paths.insert(song_id, path);
    }

    /// Tell the player whether repeat-all is on, so the session's
    /// Next button stays enabled on the last song when it wraps.
    /// Called by the Shell whenever the repeat mode changes.
    pub fn set_repeat_all(&mut self, repeat_all: bool) {
        self.repeat_all = repeat_all;
        self.update_smtc_status();
    }

    /// Hand the SMTC button channel's receiving end to the Shell
    /// (once); the Shell listens for presses on it.
    pub fn take_smtc_receiver(&mut self) -> Option<Receiver<SmtcCommand>> {
        self.smtc_rx.take()
    }

    /// Own the OS media session instead of letting the player's
    /// automatic integration run it. The automatic session only
    /// offers Play/Pause (Next/Previous are disabled by default)
    /// and carries no metadata, which is why the session showed
    /// "Unknown app" and the stream path. With the CommandManager
    /// off we enable all four buttons, route presses into the
    /// Shell over a channel, and set the track metadata ourselves.
    fn setup_smtc(&mut self, tx: Sender<SmtcCommand>) {
        let Some(player) = &self.inner else {
            return;
        };
        if let Ok(manager) = player.CommandManager() {
            let _ = manager.SetIsEnabled(false);
        }
        let Ok(smtc) = player.SystemMediaTransportControls() else {
            return;
        };
        let _ = smtc.SetIsEnabled(true);
        let _ = smtc.SetIsPlayEnabled(true);
        let _ = smtc.SetIsPauseEnabled(true);
        let _ = smtc.SetIsNextEnabled(true);
        let _ = smtc.SetIsPreviousEnabled(true);
        // Button presses arrive on a system thread; forward them to
        // the Shell over the channel (the handler cannot send a
        // Reactor message directly). Buttons we don't route are
        // ignored.
        let _ = smtc.ButtonPressed(&TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_, args| {
            let button = args.ok()?.Button()?;
            let cmd = match button {
                SystemMediaTransportControlsButton::Play => Some(SmtcCommand::Play),
                SystemMediaTransportControlsButton::Pause => Some(SmtcCommand::Pause),
                SystemMediaTransportControlsButton::Next => Some(SmtcCommand::Next),
                SystemMediaTransportControlsButton::Previous => Some(SmtcCommand::Previous),
                _ => None,
            };
            if let Some(cmd) = cmd {
                let _ = tx.send(cmd);
            }
            Ok(())
        }));
        self.smtc = Some(smtc);
    }

    /// Push the current song's metadata (title, artist, album,
    /// artwork) into the media session, so the OS controls show the
    /// track instead of the stream path. Best-effort throughout.
    fn update_smtc_metadata(&self) {
        let Some(smtc) = &self.smtc else {
            return;
        };
        let Some(song) = self.current() else {
            return;
        };
        let Ok(updater) = smtc.DisplayUpdater() else {
            return;
        };
        let _ = updater.SetType(MediaPlaybackType::Music);
        if let Ok(music) = updater.MusicProperties() {
            let _ = music.SetTitle(&HSTRING::from(song.name.clone()));
            let _ = music.SetArtist(&HSTRING::from(artist_names(song)));
            let _ =
                music.SetAlbumTitle(&HSTRING::from(song.album.name.clone().unwrap_or_default()));
            if let Some(artist) = song.artists.primary.first() {
                let _ = music.SetAlbumArtist(&HSTRING::from(artist.name.clone()));
            }
        }
        // Artwork thumbnail, from the song's image URL.
        if let Some(url) = song
            .image
            .iter()
            .find(|link| link.quality == "500x500")
            .or_else(|| song.image.last())
            .map(|link| link.url.clone())
            && let Ok(uri) = Uri::CreateUri(&HSTRING::from(url))
            && let Ok(thumb) = RandomAccessStreamReference::CreateFromUri(&uri)
        {
            let _ = updater.SetThumbnail(&thumb);
        }
        let _ = updater.Update();
    }

    /// Push the playback state (Playing / Paused / Stopped) into the
    /// media session, from the actual player state — and enable or
    /// disable Next/Previous by queue position, so the session never
    /// offers a button that does nothing.
    fn update_smtc_status(&self) {
        let Some(smtc) = &self.smtc else {
            return;
        };
        let status = if self.is_playing() {
            MediaPlaybackStatus::Playing
        } else if self.has_current() {
            MediaPlaybackStatus::Paused
        } else {
            MediaPlaybackStatus::Stopped
        };
        let _ = smtc.SetPlaybackStatus(status);
        // Next wraps to the start with repeat-all, so it stays
        // enabled on the last song then; otherwise it is off there.
        // Previous is off on the first song (it has nowhere to go).
        let has_next = self.has_current() && (!self.is_last() || self.repeat_all);
        let has_prev = self.index.map(|i| i > 0).unwrap_or(false);
        let _ = smtc.SetIsNextEnabled(has_next);
        let _ = smtc.SetIsPreviousEnabled(has_prev);
    }

    /// Push position/duration into the media session, so the OS
    /// flyout's seek bar shows where the track is and how long it
    /// is. Called on track start and on every status sync (the Tick
    /// drives that while a song is loaded, so the bar moves).
    fn update_smtc_timeline(&self) {
        let Some(smtc) = &self.smtc else {
            return;
        };
        let Ok(props) = SystemMediaTransportControlsTimelineProperties::new() else {
            return;
        };
        let position = TimeSpan {
            Duration: (self.position_secs() * 10_000_000.0) as i64,
        };
        let duration = TimeSpan {
            Duration: (self.duration_secs() * 10_000_000.0) as i64,
        };
        let _ = props.SetStartTime(TimeSpan { Duration: 0 });
        let _ = props.SetEndTime(duration);
        let _ = props.SetMinSeekTime(TimeSpan { Duration: 0 });
        let _ = props.SetMaxSeekTime(duration);
        let _ = props.SetPosition(position);
        let _ = smtc.UpdateTimelineProperties(&props);
    }

    /// Re-read the player state and push it to the media session.
    /// Called when the player stops on its own (a track ending with
    /// repeat off and autoplay off), where no play/pause handler
    /// runs to update the session — and by the Tick, so the
    /// flyout's seek bar keeps moving.
    pub fn sync_smtc_status(&self) {
        self.update_smtc_status();
        self.update_smtc_timeline();
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
    pub fn play_current(&mut self, quality: Quality) -> windows::core::Result<()> {
        let Some(player) = &self.inner else {
            return Ok(());
        };
        let Some(song) = self.current() else {
            return Ok(());
        };
        // A downloaded song plays from its local file, so it works
        // offline. Otherwise it streams from the quality URL.
        let uri = if let Some(path) = self.local_paths.get(&song.id) {
            Uri::CreateUri(&HSTRING::from(crate::download::file_uri(path)))?
        } else {
            let Some(url) = pick_url(song, quality) else {
                return Ok(());
            };
            Uri::CreateUri(&HSTRING::from(url))?
        };
        let source = MediaSource::CreateFromUri(&uri)?;
        player.SetSource(&source)?;
        // Dispose the previous track's source now that the player
        // has moved on: Close() releases its stream and network
        // resources immediately instead of leaving them to COM
        // refcounting.
        if let Some(old) = self.current_source.replace(source) {
            let _ = old.Close();
        }
        player.Play()?;
        // A new track: the session shows this song's metadata, the
        // Playing state, and the track length for the seek bar.
        self.update_smtc_metadata();
        self.update_smtc_status();
        self.update_smtc_timeline();
        Ok(())
    }

    pub fn play(&self) -> windows::core::Result<()> {
        if let Some(player) = &self.inner {
            player.Play()?;
        }
        self.update_smtc_status();
        Ok(())
    }

    pub fn pause(&self) -> windows::core::Result<()> {
        if let Some(player) = &self.inner {
            player.Pause()?;
        }
        self.update_smtc_status();
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
