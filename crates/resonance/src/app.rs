//! The app shell: TitleBar (header search), NavigationView sidebar,
//! page content, and the bottom player bar.
//!
//! WinUI design notes (Windows app design guidelines):
//! - NavigationView in Left mode is the shell's primary navigation.
//! - The title bar hosts the search box — the app's primary command.
//! - The bottom player bar is a persistent 96px media bar modeled on
//!   Spotify's Now Playing bar: track info left, controls + progress
//!   center, extras (lyrics, queue, volume) right.
//! - Spacing is 4/8/12/24; type ramps 12/14/20/28.
//!
//! windows-reactor 0.100 API notes (verified against the crate):
//! - Layout props (`margin`, `grid_row`, `grid_column`, `width`)
//!   are set on the builder *before* the terminal `.content()` /
//!   `.children()` / `.slots()` call, which returns a `View`.
//! - Multi-slot controls (TitleBar, NavigationView, NavigationViewItem)
//!   take their content through `.slots([SlotView::new(...), ...])`.
//! - `Image::source` returns `Result` (URI validation) — handle it.
//! - `FontWeight` uses `BOLD` / `SEMI_BOLD` constants.

use std::time::Duration;

use saavnapi::{Album, Artist, ArtistArgs, LaunchData, Playlist, SearchAll, Song};
use windows_reactor::*;

use crate::data;
use crate::pages;
use crate::player::{Player, Quality};
use crate::store::{Library, Material, QualitySetting, RepeatMode, Settings, Theme};
use crate::views;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Search,
    Library,
    Settings,
    Queue,
}

// NOTE: there is deliberately no `Details` page here. Details are
// an inner route of the page they were opened from (Home, Search
// or Library), not a top-level nav route — the sidebar only ever
// holds the few routes above.

/// What a details page was opened for (from a media card).
#[derive(Debug, Clone)]
pub struct DetailsTarget {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub art: Option<String>,
}

/// The data a details page shows once loaded. `songs` is the
/// playable list (for a song: just itself); `song` is set only for
/// a song detail (for the like button); `lyrics` only for songs.
/// `album` / `artist` / `playlist` carry the full model for the
/// matching kind, so each page can render its own layout.
#[derive(Debug, Clone)]
pub struct DetailsContent {
    pub song: Option<Song>,
    pub songs: Vec<Song>,
    pub lyrics: Option<String>,
    pub subtitle: String,
    pub art: Option<String>,
    pub album: Option<Album>,
    pub artist: Option<Artist>,
    pub playlist: Option<Playlist>,
}

#[derive(Debug, Clone)]
pub struct DetailsState {
    pub target: DetailsTarget,
    pub loading: bool,
    pub error: Option<String>,
    pub content: Option<DetailsContent>,
}

/// An open context menu: what it was opened for and where on the
/// window it sits (window-relative coordinates from the pointer).
#[derive(Debug, Clone)]
pub struct MenuState {
    pub target: DetailsTarget,
    pub x: f64,
    pub y: f64,
}

/// An open Properties dialog (from the context menu).
#[derive(Debug, Clone)]
pub struct PropertiesState {
    pub target: DetailsTarget,
    pub loading: bool,
    pub error: Option<String>,
    pub content: Option<DetailsContent>,
}

pub struct Shell {
    pub page: Page,
    pub details: Option<DetailsState>,
    /// Page the title-bar back button returns to (Details, Queue).
    pub back_page: Page,
    /// Card id the pointer is over (its hover buttons show).
    pub hovered_card: Option<String>,
    /// Open context menu, if any.
    pub menu: Option<MenuState>,
    /// Open Properties dialog, if any.
    pub properties: Option<PropertiesState>,
    /// The Now Playing sidebar on the right is open.
    pub now_playing_open: bool,
    /// The main artist's brief (followers, full image) for the
    /// current song, fetched when the track changes; None while it
    /// loads or when the song has no primary artist.
    pub now_playing_artist: Option<Artist>,
    /// The song id `now_playing_artist` was fetched for (stale
    /// replies for an ended song are dropped).
    pub now_playing_artist_song_id: Option<String>,
    /// The full-queue overlay panel (opened from the sidebar) is
    /// mounted over the sidebar.
    pub queue_overlay_mounted: bool,
    /// The queue overlay is at full opacity/scale (the shown state;
    /// false while it rises in or drops out).
    pub queue_overlay_shown: bool,
    /// Skeleton shimmer phase (true = dim). Flipped by the
    /// SkeletonPulse timer while any page is loading.
    pub skeleton_dim: bool,
    /// The skeleton pulse timer is running.
    pub skeleton_pulse_running: bool,
    /// Fade-in opacity for just-loaded content (0 on arrival, then
    /// animated to 1 — the content Border's opacity transition does
    /// the fade).
    pub home_fade: f64,
    pub search_fade: f64,
    pub details_fade: f64,
    pub pane_open: bool,
    pub search_text: String,
    pub search_suggestions: Vec<String>,
    pub home: Option<LaunchData>,
    pub home_loading: bool,
    pub home_error: Option<String>,
    pub search_all: Option<SearchAll>,
    pub search_loading: bool,
    pub search_error: Option<String>,
    pub library: Library,
    pub settings: Settings,
    pub player: Player,
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    /// Volume muted from the player bar (session-only); the slider
    /// shows 0 while muted and the pre-mute volume is restored.
    pub muted: bool,
    pub pre_mute_volume: f64,
}

#[derive(Clone)]
pub enum Message {
    Navigate(Page),
    TogglePane,
    PaneOpenChanged(bool),
    ToggleTheme,
    SearchTextChanged(String),
    SuggestionChosen(String),
    HomeLoaded(Result<LaunchData, String>),
    SearchAllLoaded(String, Result<SearchAll, String>),
    PlaySong(Vec<Song>, usize),
    PlayPause,
    Next,
    Previous,
    Seek(f64),
    VolumeChanged(f64),
    Tick,
    SkeletonPulse,
    HomeFadeIn,
    SearchFadeIn,
    DetailsFadeIn,
    ToggleLike(Song),
    ToggleLikeAlbum(Album),
    ToggleLikePlaylist(Playlist),
    PlayByKind(String, String),
    OpenDetails(DetailsTarget),
    DetailsLoaded(Result<DetailsContent, String>),
    CardHover(Option<String>),
    CardPressed(DetailsTarget, PointerEventInfo),
    OpenMenu(DetailsTarget, f64, f64),
    CloseOverlay,
    MenuPlay(DetailsTarget),
    MenuPlayNext(DetailsTarget),
    MenuProperties(DetailsTarget),
    PlayNext(Vec<Song>),
    PropertiesLoaded(Result<DetailsContent, String>),
    CloseProperties,
    OpenQueue,
    ToggleNowPlaying,
    ArtistBriefLoaded(String, Result<Artist, String>),
    OpenQueueOverlay,
    QueueOverlayShown,
    CloseQueueOverlay,
    QueueOverlayHidden,
    ToggleShuffle,
    CycleRepeat,
    ToggleMute,
    Back,
    SetTheme(Theme),
    SetMaterial(Material),
    SetQuality(QualitySetting),
    SetAutoplay(bool),
    RetryHome,
    Noop,
}

/// Load the Home launch data in the background.
fn spawn_home_load(context: &ComponentContext<Shell>) {
    context.spawn_background(move |_| {
        let result =
            data::runtime_handle().block_on(async { data::client().get_launch_data(None).await });
        Message::HomeLoaded(result.map_err(|err| err.to_string()))
    });
}

/// Fetch a song/album/playlist/artist's songs: a song on its own,
/// an album's songs, an artist's top songs, or a playlist's songs.
async fn fetch_songs_by_kind(
    client: &saavnapi::SaavnClient,
    kind: &str,
    id: &str,
) -> saavnapi::Result<Vec<Song>> {
    match kind {
        "song" => client.get_songs_by_ids(&[id]).await,
        "album" => client
            .get_album_by_id(id)
            .await
            .map(|album| album.songs.unwrap_or_default()),
        "artist" => {
            client
                .get_artist_top_songs(id, "", 0, None, None, None)
                .await
        }
        _ => client
            .get_playlist_by_id(id, 50, 0)
            .await
            .map(|playlist| playlist.songs.unwrap_or_default()),
    }
}

/// Fetch a song/album/playlist/artist's songs in the background and
/// start the queue at the first song. Failures are silent (Noop).
fn spawn_play_by_kind(context: &ComponentContext<Shell>, kind: String, id: String) {
    context.spawn_background(move |_| {
        let result = data::runtime_handle()
            .block_on(async { fetch_songs_by_kind(&data::client(), &kind, &id).await });
        match result {
            Ok(queue) if !queue.is_empty() => Message::PlaySong(queue, 0),
            _ => Message::Noop,
        }
    });
}

/// Fetch a song/album/playlist/artist's songs in the background to
/// queue them right after the current song (Play next).
fn spawn_play_next_by_kind(context: &ComponentContext<Shell>, kind: String, id: String) {
    context.spawn_background(move |_| {
        let result = data::runtime_handle()
            .block_on(async { fetch_songs_by_kind(&data::client(), &kind, &id).await });
        match result {
            Ok(queue) if !queue.is_empty() => Message::PlayNext(queue),
            _ => Message::Noop,
        }
    });
}

/// Fetch a details page's content: a song (with lyrics when
/// available), an album, an artist's top songs, or a playlist.
async fn load_details(
    client: &saavnapi::SaavnClient,
    target: &DetailsTarget,
) -> saavnapi::Result<DetailsContent> {
    match target.kind.as_str() {
        "song" => {
            let songs = client.get_songs_by_ids(&[target.id.as_str()]).await?;
            let song = songs.into_iter().next();
            let mut lyrics = None;
            if let Some(song) = &song
                && let Some(lyrics_id) = &song.lyrics_id
                && let Ok(Some(found)) = client.get_lyrics(lyrics_id).await
            {
                lyrics = Some(found.text.replace("<br><br>", "\n\n").replace("<br>", "\n"));
            }
            let (subtitle, art) = match &song {
                Some(song) => (
                    views::artist_names(song),
                    views::art_url(&song.image, "500x500"),
                ),
                None => (target.subtitle.clone(), target.art.clone()),
            };
            let songs = song.clone().into_iter().collect();
            Ok(DetailsContent {
                song,
                songs,
                lyrics,
                subtitle,
                art,
                album: None,
                artist: None,
                playlist: None,
            })
        }
        "album" => {
            let album = client.get_album_by_id(&target.id).await?;
            let subtitle = album
                .artists
                .primary
                .iter()
                .map(|artist| artist.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let songs = album.songs.clone().unwrap_or_default();
            let art = views::art_url(&album.image, "500x500");
            Ok(DetailsContent {
                song: None,
                songs,
                lyrics: None,
                subtitle,
                art,
                album: Some(album),
                artist: None,
                playlist: None,
            })
        }
        "artist" => {
            let artist = client
                .get_artist_by_id(&target.id, ArtistArgs::default())
                .await?;
            let subtitle = artist
                .follower_count
                .map(|count| format!("{count} followers"))
                .unwrap_or_default();
            let songs = artist.top_songs.clone().unwrap_or_default();
            let art = views::art_url(&artist.image, "500x500");
            Ok(DetailsContent {
                song: None,
                songs,
                lyrics: None,
                subtitle,
                art,
                album: None,
                artist: Some(artist),
                playlist: None,
            })
        }
        _ => {
            let playlist = client.get_playlist_by_id(&target.id, 100, 0).await?;
            let subtitle = playlist.description.clone().unwrap_or_default();
            let songs = playlist.songs.clone().unwrap_or_default();
            let art = views::art_url(&playlist.image, "500x500");
            Ok(DetailsContent {
                song: None,
                songs,
                lyrics: None,
                subtitle,
                art,
                album: None,
                artist: None,
                playlist: Some(playlist),
            })
        }
    }
}

/// Open a details page and load its content in the background.
fn spawn_details_load(context: &ComponentContext<Shell>, target: DetailsTarget) {
    context.spawn_background(move |_| {
        let result =
            data::runtime_handle().block_on(async { load_details(&data::client(), &target).await });
        Message::DetailsLoaded(result.map_err(|err| err.to_string()))
    });
}

/// Load a Properties dialog's content in the background (same data
/// as the details page).
fn spawn_properties_load(context: &ComponentContext<Shell>, target: DetailsTarget) {
    context.spawn_background(move |_| {
        let result =
            data::runtime_handle().block_on(async { load_details(&data::client(), &target).await });
        Message::PropertiesLoaded(result.map_err(|err| err.to_string()))
    });
}

/// Poll the player every 500ms while a song is loaded.
fn spawn_tick(context: &ComponentContext<Shell>) {
    context.spawn_background(move |_| {
        std::thread::sleep(Duration::from_millis(500));
        Message::Tick
    });
}

/// One shimmer step: sleeps, then flips the skeleton phase. The
/// SkeletonPulse handler re-spawns this while anything is loading.
fn spawn_skeleton_pulse(context: &ComponentContext<Shell>) {
    context.spawn_background(move |_| {
        std::thread::sleep(Duration::from_millis(650));
        Message::SkeletonPulse
    });
}

/// A beat after content arrives, raise its fade to 1 so the page
/// Border's opacity transition animates the fade-in. The short sleep
/// lets the opacity-0 frame actually paint first, so the transition
/// sees a real change.
fn spawn_fade_in(context: &ComponentContext<Shell>, message: Message) {
    context.spawn_background(move |_| {
        std::thread::sleep(Duration::from_millis(30));
        message
    });
}

fn nav_item(tag: &str, label: &str, icon: Symbol, selected: bool) -> KeyedView {
    KeyedView::new(
        tag.to_string(),
        NavigationViewItem::new()
            .tag(tag)
            .is_selected(selected)
            .selects_on_invoked(true)
            .slots([
                SlotView::new(NavigationViewItemSlot::Content, label.to_string()),
                SlotView::new(NavigationViewItemSlot::Icon, SymbolIcon::new().symbol(icon)),
            ]),
    )
}

impl Component for Shell {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let settings = Settings::load();
        let library = Library::load();
        let player = Player::new();
        player.set_volume(settings.volume);

        // Home data loads in the background as the shell opens, so
        // the skeleton shimmer starts with it.
        spawn_home_load(context);
        spawn_skeleton_pulse(context);

        Self {
            page: Page::Home,
            details: None,
            back_page: Page::Home,
            hovered_card: None,
            menu: None,
            properties: None,
            now_playing_open: false,
            now_playing_artist: None,
            now_playing_artist_song_id: None,
            queue_overlay_mounted: false,
            queue_overlay_shown: false,
            skeleton_dim: false,
            skeleton_pulse_running: true,
            home_fade: 1.0,
            search_fade: 1.0,
            details_fade: 1.0,
            pane_open: true,
            search_text: String::new(),
            search_suggestions: Vec::new(),
            home: None,
            home_loading: true,
            home_error: None,
            search_all: None,
            search_loading: false,
            search_error: None,
            library,
            settings,
            player,
            position: 0.0,
            duration: 0.0,
            playing: false,
            muted: false,
            pre_mute_volume: 0.8,
        }
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::Navigate(page) => {
                self.page = page;
                // Switching top-level pages leaves a details drill-in.
                self.details = None;
            }
            Message::TogglePane => self.pane_open = !self.pane_open,
            Message::PaneOpenChanged(open) => self.pane_open = open,
            Message::ToggleTheme => {
                self.settings.theme = match self.settings.theme {
                    Theme::Dark => Theme::Light,
                    Theme::Light => Theme::System,
                    Theme::System => Theme::Dark,
                };
                self.settings.save();
            }
            Message::SearchTextChanged(text) => {
                self.search_text = text.clone();
                // Typing searches whatever is typed, live: one
                // `search_all` call per keystroke feeds BOTH the
                // autocomplete suggestions and the full results, so
                // results never wait for a suggestion to be picked.
                // The query rides along so a slow reply can't
                // overwrite results for newer text.
                let query = text.trim().to_string();
                if query.len() >= 2 {
                    // Typing shows search results, not a stale
                    // details drill-in from another page.
                    self.details = None;
                    if self.page != Page::Search {
                        self.page = Page::Search;
                    }
                    self.search_loading = true;
                    self.search_error = None;
                    self.ensure_skeleton_pulse(context);
                    context.spawn_background(move |_| {
                        let result = data::runtime_handle()
                            .block_on(async { data::client().search_all(&query).await });
                        Message::SearchAllLoaded(query, result.map_err(|err| err.to_string()))
                    });
                } else {
                    self.search_suggestions.clear();
                    self.search_all = None;
                    self.search_error = None;
                    self.search_loading = false;
                }
            }
            Message::SuggestionChosen(text) => {
                self.search_text = text;
                self.submit_search(context);
            }
            Message::HomeLoaded(result) => {
                self.home_loading = false;
                match result {
                    Ok(data) => {
                        self.home = Some(data);
                        // Content arrives invisible and fades in (the
                        // page Border's opacity transition animates
                        // the 0 -> 1 change a beat later).
                        self.home_fade = 0.0;
                        spawn_fade_in(context, Message::HomeFadeIn);
                    }
                    Err(err) => self.home_error = Some(err),
                }
            }
            Message::SearchAllLoaded(query, result) => {
                if query == self.search_text.trim() {
                    self.search_loading = false;
                    match result {
                        Ok(all) => {
                            self.search_suggestions = all
                                .top_query
                                .results
                                .iter()
                                .map(|hit| hit.title.clone())
                                .take(8)
                                .collect();
                            self.search_all = Some(all);
                            self.search_fade = 0.0;
                            spawn_fade_in(context, Message::SearchFadeIn);
                        }
                        Err(err) => self.search_error = Some(err),
                    }
                }
            }
            Message::PlaySong(queue, index) => {
                let quality = Quality::from_setting(self.settings.quality);
                self.player.load_queue(queue, index, quality);
                // after_track_change does the rest: recents, the
                // playing/position/duration refresh, opening the Now
                // Playing sidebar, the artist-brief fetch, the tick.
                self.after_track_change(context);
            }
            Message::PlayPause => {
                self.playing = self.player.toggle();
                if self.playing {
                    spawn_tick(context);
                }
            }
            Message::Next => {
                let quality = Quality::from_setting(self.settings.quality);
                // Repeat-all wraps to the start of the queue; plain
                // Next stops at the last song.
                if self.settings.repeat == RepeatMode::All && self.player.is_last() {
                    self.player.play_index(0, quality);
                } else {
                    self.player.next(quality);
                }
                self.after_track_change(context);
            }
            Message::Previous => {
                let quality = Quality::from_setting(self.settings.quality);
                self.player.previous(quality);
                self.after_track_change(context);
            }
            Message::Seek(secs) => {
                // Deadband: the slider re-fires when Tick updates its
                // value; ignore seeks that land where we already are.
                if (secs - self.position).abs() > 1.0 {
                    self.player.seek(secs);
                    self.position = secs;
                }
            }
            Message::VolumeChanged(volume) => {
                // Dragging the slider un-mutes; dragging to 0 mutes.
                self.muted = volume <= 0.0;
                self.settings.volume = volume;
                self.settings.save();
                self.player.set_volume(volume);
            }
            Message::ToggleMute => {
                if self.muted {
                    self.muted = false;
                    let volume = if self.pre_mute_volume > 0.0 {
                        self.pre_mute_volume
                    } else {
                        0.8
                    };
                    self.settings.volume = volume;
                    self.settings.save();
                    self.player.set_volume(volume);
                } else {
                    self.pre_mute_volume = self.settings.volume;
                    self.muted = true;
                    self.player.set_volume(0.0);
                }
            }
            Message::ToggleShuffle => {
                self.settings.shuffle = !self.settings.shuffle;
                self.settings.save();
                if self.settings.shuffle {
                    self.player.shuffle_upcoming();
                }
            }
            Message::CycleRepeat => {
                self.settings.repeat = self.settings.repeat.next();
                self.settings.save();
            }
            Message::OpenQueue => {
                if self.page != Page::Queue {
                    self.back_page = self.page;
                }
                // The queue replaces whatever is showing, details
                // drill-in included — and the Now Playing sidebar
                // closes, since the full queue is the queue view.
                self.details = None;
                self.page = Page::Queue;
                self.now_playing_open = false;
                self.queue_overlay_mounted = false;
                self.queue_overlay_shown = false;
            }
            Message::ToggleNowPlaying => {
                self.now_playing_open = !self.now_playing_open;
                if !self.now_playing_open {
                    // The overlay lives over the sidebar; hiding the
                    // sidebar hides it too.
                    self.queue_overlay_mounted = false;
                    self.queue_overlay_shown = false;
                }
            }
            Message::ArtistBriefLoaded(song_id, result) => {
                // Only the current song's artist brief applies; a
                // late reply for a song that already ended is
                // dropped. A failed fetch just leaves the brief at
                // name-only (the song already carries that).
                if let Ok(artist) = result
                    && self.player.current().is_some_and(|s| s.id == song_id)
                {
                    self.now_playing_artist = Some(artist);
                }
            }
            Message::OpenQueueOverlay => {
                // Mount the panel hidden (opacity 0, scale 0.96),
                // then a beat later raise it — the Border's
                // transitions animate the rise.
                self.queue_overlay_mounted = true;
                self.queue_overlay_shown = false;
                spawn_fade_in(context, Message::QueueOverlayShown);
            }
            Message::QueueOverlayShown => {
                self.queue_overlay_shown = true;
            }
            Message::CloseQueueOverlay => {
                // Animate down (opacity 0, scale 0.96), then
                // unmount once the 300ms transition has run.
                self.queue_overlay_shown = false;
                context.spawn_background(move |_| {
                    std::thread::sleep(Duration::from_millis(300));
                    Message::QueueOverlayHidden
                });
            }
            Message::QueueOverlayHidden => {
                self.queue_overlay_mounted = false;
            }
            Message::SetTheme(theme) => {
                self.settings.theme = theme;
                self.settings.save();
            }
            Message::SetMaterial(material) => {
                self.settings.material = material;
                self.settings.save();
            }
            Message::SetQuality(quality) => {
                self.settings.quality = quality;
                self.settings.save();
            }
            Message::SetAutoplay(autoplay) => {
                self.settings.autoplay = autoplay;
                self.settings.save();
            }
            Message::Tick => {
                self.position = self.player.position_secs();
                self.duration = self.player.duration_secs();
                self.playing = self.player.is_playing();
                // MediaPlayer doesn't auto-advance our queue. At the
                // end of a track, repeat wins over autoplay: repeat-one
                // restarts the song, repeat-all wraps to the start of
                // the queue. With repeat off, autoplay decides whether
                // the queue advances at all.
                if self.playing && self.duration > 5.0 && self.position >= self.duration - 0.3 {
                    let quality = Quality::from_setting(self.settings.quality);
                    match self.settings.repeat {
                        RepeatMode::One => {
                            self.player.restart();
                            self.after_track_change(context);
                        }
                        RepeatMode::All => {
                            if self.player.is_last() {
                                self.player.play_index(0, quality);
                            } else {
                                self.player.next(quality);
                            }
                            self.after_track_change(context);
                        }
                        RepeatMode::Off => {
                            if self.settings.autoplay {
                                self.player.next(quality);
                                self.after_track_change(context);
                            }
                        }
                    }
                } else if self.player.has_current() {
                    spawn_tick(context);
                }
            }
            Message::SkeletonPulse => {
                // Flip the shimmer phase; keep the loop going only
                // while a skeleton is still on screen.
                self.skeleton_dim = !self.skeleton_dim;
                if self.any_loading() {
                    spawn_skeleton_pulse(context);
                } else {
                    self.skeleton_pulse_running = false;
                }
            }
            Message::HomeFadeIn => {
                self.home_fade = 1.0;
            }
            Message::SearchFadeIn => {
                self.search_fade = 1.0;
            }
            Message::DetailsFadeIn => {
                self.details_fade = 1.0;
            }
            Message::ToggleLike(song) => {
                self.library.toggle_like(song);
            }
            Message::ToggleLikeAlbum(album) => {
                self.library.toggle_like_album(album);
            }
            Message::ToggleLikePlaylist(playlist) => {
                self.library.toggle_like_playlist(playlist);
            }
            Message::PlayByKind(kind, id) => {
                spawn_play_by_kind(context, kind, id);
            }
            Message::OpenDetails(target) => {
                self.open_details(context, target);
            }
            Message::DetailsLoaded(result) => {
                if let Some(details) = &mut self.details {
                    details.loading = false;
                    match result {
                        Ok(content) => {
                            details.content = Some(content);
                            self.details_fade = 0.0;
                            spawn_fade_in(context, Message::DetailsFadeIn);
                        }
                        Err(err) => details.error = Some(err),
                    }
                }
            }
            Message::CardHover(id) => {
                self.hovered_card = id;
            }
            Message::CardPressed(target, info) => {
                if info.is_right_button_pressed {
                    self.open_menu(target, info.window_x, info.window_y);
                } else if info.is_left_button_pressed {
                    // A song card just plays the song; every other
                    // kind opens its own details page.
                    if target.kind == "song" {
                        spawn_play_by_kind(context, target.kind, target.id);
                    } else {
                        self.open_details(context, target);
                    }
                }
            }
            Message::OpenMenu(target, x, y) => {
                self.open_menu(target, x, y);
            }
            Message::CloseOverlay => {
                self.menu = None;
                self.properties = None;
            }
            Message::MenuPlay(target) => {
                self.menu = None;
                spawn_play_by_kind(context, target.kind, target.id);
            }
            Message::MenuPlayNext(target) => {
                self.menu = None;
                spawn_play_next_by_kind(context, target.kind, target.id);
            }
            Message::MenuProperties(target) => {
                self.menu = None;
                self.properties = Some(PropertiesState {
                    target: target.clone(),
                    loading: true,
                    error: None,
                    content: None,
                });
                spawn_properties_load(context, target);
            }
            Message::PlayNext(songs) => {
                if songs.is_empty() {
                    // Nothing queued; ignore.
                } else if self.player.has_current() {
                    self.player.insert_next(songs);
                } else {
                    // Nothing playing: play the queued songs now.
                    let quality = Quality::from_setting(self.settings.quality);
                    self.player.load_queue(songs, 0, quality);
                    if let Some(song) = self.player.current() {
                        self.library.push_recent(song.clone());
                    }
                    self.playing = true;
                    self.position = 0.0;
                    self.duration = self.player.duration_secs();
                    spawn_tick(context);
                }
            }
            Message::PropertiesLoaded(result) => {
                if let Some(properties) = &mut self.properties {
                    properties.loading = false;
                    match result {
                        Ok(content) => properties.content = Some(content),
                        Err(err) => properties.error = Some(err),
                    }
                }
            }
            Message::CloseProperties => {
                self.properties = None;
            }
            Message::Back => {
                if self.details.is_some() {
                    // Close the inner details route; stay on the
                    // same page it was opened from.
                    self.details = None;
                } else if self.page == Page::Queue {
                    self.page = self.back_page;
                }
            }
            Message::RetryHome => {
                self.home_loading = true;
                self.home_error = None;
                self.ensure_skeleton_pulse(context);
                spawn_home_load(context);
            }
            Message::Noop => {}
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        let theme = match self.settings.theme {
            Theme::Dark => WindowTheme::Dark,
            Theme::Light => WindowTheme::Light,
            Theme::System => WindowTheme::System,
        };
        let backdrop = match self.settings.material {
            Material::Mica => WindowBackdrop::Mica,
            Material::MicaAlt => WindowBackdrop::MicaAlt,
            Material::Acrylic => WindowBackdrop::Acrylic,
            Material::None => WindowBackdrop::None,
        };
        context.window_visuals(
            WindowVisuals::new()
                .client_size(1280.0, 800.0)
                .theme(theme)
                .backdrop(backdrop),
        );

        let theme_label = match self.settings.theme {
            Theme::Dark => "Theme: Dark",
            Theme::Light => "Theme: Light",
            Theme::System => "Theme: System",
        };

        let on_drill_in = self.details.is_some() || self.page == Page::Queue;
        let title_bar = TitleBar::new()
            .preferred_height(WindowTitleBarHeight::Tall)
            .title("Resonance")
            .is_back_button_visible(on_drill_in)
            .is_back_button_enabled(on_drill_in)
            .on_back_requested(context.message(Message::Back))
            .is_pane_toggle_button_visible(true)
            .on_pane_toggle_requested(context.message(Message::TogglePane))
            .slots([
                SlotView::new(
                    TitleBarSlot::Content,
                    TextBox::new()
                        .text(self.search_text.clone())
                        .placeholder_text("Search songs, albums, artists...")
                        .width(360.0)
                        .on_text_changed(
                            context.callback(|value: String| Message::SearchTextChanged(value)),
                        ),
                ),
                SlotView::new(
                    TitleBarSlot::RightHeader,
                    Button::new()
                        .on_click(context.message(Message::ToggleTheme))
                        .content(theme_label),
                ),
            ]);

        let page_view: View = match self.page {
            Page::Home => pages::home::view(self, context),
            Page::Search => pages::search::view(self, context),
            Page::Library => pages::library::view(self, context),
            Page::Settings => pages::settings::view(self, context),
            Page::Queue => pages::queue::view(self, context),
        };

        // Settings sits in the pane footer, at the very bottom of
        // the sidebar — not with the other navigation items. (0.100
        // has no FooterMenuItems slot.) It is a Border, not a Button:
        // a Button in the pane footer never answered its click, while
        // Border pointer presses are the input path that works
        // everywhere else in the app. Selected state is a card
        // background fill, so it can never look selected when it
        // isn't.
        let settings_selected = self.page == Page::Settings;
        let settings_footer = Border::new()
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .background(if settings_selected {
                Brush::Theme(ThemeBrush::CardBackground)
            } else {
                Brush::Solid(Color::transparent())
            })
            .corner_radius(CornerRadius::uniform(4.0))
            .margin(8.0)
            .padding(Thickness::new(12.0, 8.0, 12.0, 8.0))
            .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                if info.is_left_button_pressed {
                    Message::Navigate(Page::Settings)
                } else {
                    Message::Noop
                }
            }))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(12.0)
                    .children((
                        SymbolIcon::new().symbol(Symbol::Setting),
                        TextBlock::new()
                            .text("Settings")
                            .font_size(14.0)
                            .font_weight(if settings_selected {
                                FontWeight::SEMI_BOLD
                            } else {
                                FontWeight::NORMAL
                            }),
                    )),
            );

        let navigation = NavigationView::new()
            .pane_display_mode(NavigationViewPaneDisplayMode::Left)
            .is_pane_toggle_button_visible(false)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .is_settings_visible(false)
            .always_show_header(false)
            .is_pane_open(self.pane_open)
            .on_is_pane_open_changed(context.callback(Message::PaneOpenChanged))
            .on_selected_tag_changed(
                context.callback(|tag: Option<String>| match tag.as_deref() {
                    Some("home") => Message::Navigate(Page::Home),
                    Some("search") => Message::Navigate(Page::Search),
                    Some("library") => Message::Navigate(Page::Library),
                    // A null or unknown tag means the selection was
                    // cleared — a drill-in page (Queue) or the Settings
                    // footer, neither of which is a nav item — and that
                    // is not a navigation. Mapping null to Home used to
                    // bounce every drill-in straight back to Home the
                    // moment it opened, so the click looked like it did
                    // nothing.
                    _ => Message::Noop,
                }),
            )
            .grid_row(1)
            .slots([
                SlotView::collection(
                    NavigationViewSlot::MenuItems,
                    vec![
                        nav_item("home", "Home", Symbol::Home, self.page == Page::Home),
                        nav_item("search", "Search", Symbol::Find, self.page == Page::Search),
                        nav_item(
                            "library",
                            "Library",
                            Symbol::Audio,
                            self.page == Page::Library,
                        ),
                    ],
                ),
                SlotView::new(NavigationViewSlot::PaneFooter, settings_footer),
                SlotView::new(NavigationViewSlot::Content, page_view),
            ]);

        let player_bar = self.player_bar(context);

        // The Now Playing sidebar rides in the shell grid's content
        // row, painted above the NavigationView (it covers the right
        // of the page) and below the menu/dialog overlay. It lives in
        // row 1, so it never covers the title bar or the player bar.
        let mut shell_children: Vec<KeyedView> = vec![
            KeyedView::new("titlebar", title_bar),
            KeyedView::new("navigation", navigation),
            KeyedView::new("playerbar", player_bar),
        ];
        if self.now_playing_open {
            shell_children.push(KeyedView::new(
                "nowplaying",
                self.now_playing_sidebar(context),
            ));
        }
        if self.queue_overlay_mounted {
            // Painted after the sidebar, so it covers it.
            shell_children.push(KeyedView::new("queueoverlay", self.queue_overlay(context)));
        }
        let shell_grid = Grid::new()
            .rows([
                GridLength::Auto,
                GridLength::Star(1.0),
                GridLength::Pixel(96.0),
            ])
            .keyed_children(shell_children);

        // Overlay layer (context menu / Properties dialog) sits in
        // the same root cell, painted above the shell. It only
        // exists while a menu or dialog is open.
        if self.menu.is_none() && self.properties.is_none() {
            return shell_grid;
        }
        let mut root: Vec<KeyedView> = vec![KeyedView::new("shell", shell_grid)];
        root.push(KeyedView::new("overlay", self.overlay_view(context)));
        Grid::new()
            .rows([GridLength::Star(1.0)])
            .keyed_children(root)
    }
}

impl Shell {
    /// The overlay layer: a click-away layer plus the open menu /
    /// Properties dialog. The click-away layer is a *sibling* of the
    /// menu and dialog (not their parent), so presses on the menu
    /// itself never reach it.
    fn overlay_view(&self, context: &mut ViewContext<Self>) -> View {
        let mut layers: Vec<KeyedView> = vec![KeyedView::new(
            "dismiss",
            Border::new()
                .background(Brush::Solid(Color::transparent()))
                .on_pointer_pressed(
                    context.callback(|_info: PointerEventInfo| Message::CloseOverlay),
                ),
        )];
        if let Some(menu) = &self.menu {
            layers.push(KeyedView::new("menu", self.menu_view(menu, context)));
        }
        if let Some(properties) = &self.properties {
            layers.push(KeyedView::new(
                "properties",
                self.properties_view(properties, context),
            ));
        }
        Grid::new().keyed_children(layers)
    }

    /// The context menu: Play, Play next, Properties.
    fn menu_view(&self, menu: &MenuState, context: &mut ViewContext<Self>) -> View {
        let target = menu.target.clone();
        let play_target = target.clone();
        let next_target = target.clone();
        Border::new()
            .width(220.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .vertical_alignment(VerticalAlignment::Top)
            .margin(Thickness::new(menu.x, menu.y, 0.0, 0.0))
            .background(Brush::Theme(ThemeBrush::CardBackground))
            .border_brush(Brush::Theme(ThemeBrush::CardStroke))
            .border_thickness(1.0)
            .corner_radius(CornerRadius::uniform(8.0))
            .padding(4.0)
            .content(StackPanel::new().spacing(2.0).children((
                menu_row("Play", Message::MenuPlay(play_target), context),
                menu_row("Play next", Message::MenuPlayNext(next_target), context),
                menu_row("Properties", Message::MenuProperties(target), context),
            )))
    }

    /// The Properties dialog: title, type, subtitle, and the loaded
    /// details (duration/year for songs, song count for collections).
    fn properties_view(
        &self,
        properties: &PropertiesState,
        context: &mut ViewContext<Self>,
    ) -> View {
        let target = &properties.target;
        let kind_label = match target.kind.as_str() {
            "song" => "Song",
            "album" => "Album",
            "artist" => "Artist",
            _ => "Playlist",
        };
        let mut rows: Vec<KeyedView> = vec![
            KeyedView::new(
                "title",
                TextBlock::new()
                    .text(target.title.clone())
                    .font_size(20.0)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .text_wrapping(TextWrapping::Wrap),
            ),
            KeyedView::new("type", property_row("Type", kind_label)),
            KeyedView::new("subtitle", property_row("Details", &target.subtitle)),
        ];
        if properties.loading {
            rows.push(KeyedView::new(
                "loading",
                ProgressRing::new().is_active(true),
            ));
        } else if let Some(err) = &properties.error {
            rows.push(KeyedView::new(
                "error",
                TextBlock::new()
                    .text(format!("Couldn't load details: {err}"))
                    .font_size(14.0)
                    .opacity(0.7)
                    .text_wrapping(TextWrapping::Wrap),
            ));
        } else if let Some(content) = &properties.content {
            if let Some(song) = &content.song {
                if let Some(duration) = song.duration {
                    rows.push(KeyedView::new(
                        "duration",
                        property_row("Duration", &views::fmt_time(duration as f64)),
                    ));
                }
                if let Some(year) = &song.year {
                    rows.push(KeyedView::new("year", property_row("Year", year)));
                }
                if let Some(album) = &song.album.name {
                    rows.push(KeyedView::new("album", property_row("Album", album)));
                }
            } else if !content.songs.is_empty() {
                rows.push(KeyedView::new(
                    "count",
                    property_row("Songs", &content.songs.len().to_string()),
                ));
            }
        }
        rows.push(KeyedView::new(
            "close",
            Button::new()
                .on_click(context.message(Message::CloseProperties))
                .content("Close"),
        ));
        Border::new()
            .width(380.0)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center)
            .background(Brush::Theme(ThemeBrush::CardBackground))
            .border_brush(Brush::Theme(ThemeBrush::CardStroke))
            .border_thickness(1.0)
            .corner_radius(CornerRadius::uniform(12.0))
            .padding(24.0)
            .content(StackPanel::new().spacing(12.0).keyed_children(rows))
    }
}

/// One context-menu row: a full-width left-aligned text button.
fn menu_row(label: &str, message: Message, context: &mut ViewContext<Shell>) -> View {
    Button::new()
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .horizontal_content_alignment(HorizontalAlignment::Left)
        .on_click(context.message(message))
        .content(label)
}

/// One Properties label/value pair.
fn property_row(label: &str, value: &str) -> View {
    StackPanel::new().spacing(2.0).children((
        TextBlock::new().text(label).font_size(12.0).opacity(0.7),
        TextBlock::new()
            .text(value)
            .font_size(14.0)
            .text_wrapping(TextWrapping::Wrap),
    ))
}

impl Shell {
    /// Open a details page for a card (shared by card clicks and
    /// the context menu's Play is separate — this is the drill-in).
    fn open_details(&mut self, context: &ComponentContext<Self>, target: DetailsTarget) {
        // Details is an inner route of the current page, not a
        // top-level nav route: the page stays mounted (and stays
        // selected in the sidebar) and the details render inside
        // it. Changing `page` here used to deselect the sidebar
        // item, and the selection-cleared event bounced straight
        // back to Home — so the click looked like it did nothing.
        self.menu = None;
        self.details = Some(DetailsState {
            target: target.clone(),
            loading: true,
            error: None,
            content: None,
        });
        self.ensure_skeleton_pulse(context);
        spawn_details_load(context, target);
    }

    /// Open the context menu at window coordinates, clamped so the
    /// ~220x150 menu stays inside the 1280x800 window.
    fn open_menu(&mut self, target: DetailsTarget, x: f64, y: f64) {
        let x = x.clamp(0.0, 1280.0 - 230.0);
        let y = y.clamp(0.0, 800.0 - 160.0);
        self.menu = Some(MenuState { target, x, y });
    }

    /// Run the current header/page query through `search_all` and
    /// show the Search page. Shared by the header button and by
    /// choosing a suggestion.
    fn submit_search(&mut self, context: &ComponentContext<Self>) {
        let query = self.search_text.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.page = Page::Search;
        // Choosing a suggestion shows results, not a stale
        // details drill-in.
        self.details = None;
        self.search_loading = true;
        self.search_error = None;
        self.ensure_skeleton_pulse(context);
        context.spawn_background(move |_| {
            let result =
                data::runtime_handle().block_on(async { data::client().search_all(&query).await });
            Message::SearchAllLoaded(query, result.map_err(|err| err.to_string()))
        });
    }

    /// Shared bookkeeping after Next/Previous/auto-advance.
    /// True while any page is showing a skeleton.
    fn any_loading(&self) -> bool {
        self.home_loading || self.search_loading || self.details.as_ref().is_some_and(|d| d.loading)
    }

    /// Start the skeleton shimmer if it isn't already running and
    /// something is loading. Several load sites share the one loop;
    /// it stops itself when nothing is loading.
    fn ensure_skeleton_pulse(&mut self, context: &ComponentContext<Self>) {
        if !self.skeleton_pulse_running && self.any_loading() {
            self.skeleton_pulse_running = true;
            spawn_skeleton_pulse(context);
        }
    }

    /// The skeleton fills' current opacity (the shimmer phase).
    pub fn skeleton_opacity(&self) -> f64 {
        if self.skeleton_dim {
            views::SKELETON_DIM
        } else {
            views::SKELETON_BRIGHT
        }
    }

    fn after_track_change(&mut self, context: &ComponentContext<Self>) {
        if let Some(song) = self.player.current() {
            self.library.push_recent(song.clone());
            // A new song means a new main artist: drop the old brief
            // and fetch this song's artist (followers + full image
            // for the sidebar's artist brief). Same song (repeat)
            // does not re-fetch.
            if self.now_playing_artist_song_id.as_deref() != Some(song.id.as_str()) {
                self.now_playing_artist = None;
                self.now_playing_artist_song_id = Some(song.id.clone());
                if let Some(artist) = song.artists.primary.first() {
                    let artist_id = artist.id.clone();
                    let song_id = song.id.clone();
                    context.spawn_background(move |_| {
                        let result = data::runtime_handle().block_on(async {
                            data::client()
                                .get_artist_by_id(&artist_id, ArtistArgs::default())
                                .await
                        });
                        Message::ArtistBriefLoaded(song_id, result.map_err(|err| err.to_string()))
                    });
                }
            }
        }
        self.playing = self.player.is_playing();
        self.position = self.player.position_secs();
        self.duration = self.player.duration_secs();
        if self.player.has_current() {
            // A track starting opens the Now Playing sidebar, so the
            // details are there the moment media plays. Hiding it is
            // a toggle; the next track start opens it again.
            self.now_playing_open = true;
            spawn_tick(context);
        }
    }

    /// The persistent bottom player bar (96px), modeled on Spotify's
    /// Now Playing bar. Three zones: track info (left), controls +
    /// progress stacked (center), extras (right).
    ///
    /// The play/pause button is a filled circular Border (Button has
    /// no corner radius in windows-reactor 0.100); Border is also the
    /// only control with pointer events, so it answers left presses.
    fn player_bar(&self, context: &mut ViewContext<Self>) -> View {
        let has_current = self.player.has_current();
        let current = self.player.current().cloned();

        // --- Left: 56px art, title (one line), artists (up to two
        // lines of clickable names), add-to-liked button. ---
        let (art, title, artist_view): (Option<String>, String, View) = match &current {
            Some(song) => (
                views::art_url(&song.image, "150x150"),
                song.name.clone(),
                views::artist_links(&song.artists, 2, 44, context),
            ),
            None => (
                None,
                "Nothing playing".to_string(),
                TextBlock::new()
                    .text("Search or open Home to play a song")
                    .font_size(12.0)
                    .opacity(0.7)
                    .text_wrapping(TextWrapping::Wrap)
                    .max_lines(2)
                    .into(),
            ),
        };
        let art_view: View = match art {
            Some(url) => views::image_from_url(&url)
                .width(56.0)
                .height(56.0)
                .stretch(Stretch::UniformToFill)
                .grid_column(0)
                .into(),
            None => Border::new()
                .width(56.0)
                .height(56.0)
                .grid_column(0)
                .content(
                    SymbolIcon::new()
                        .symbol(Symbol::Audio)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .vertical_alignment(VerticalAlignment::Center),
                ),
        };
        let liked = current
            .as_ref()
            .is_some_and(|song| self.library.is_liked(&song.id));
        let like_button: View = match &current {
            Some(song) => Button::new()
                .grid_column(2)
                .on_click(context.message(Message::ToggleLike(song.clone())))
                .content(SymbolIcon::new().symbol(if liked {
                    Symbol::Favorite
                } else {
                    Symbol::Add
                })),
            None => Button::new()
                .grid_column(2)
                .is_enabled(false)
                .content(SymbolIcon::new().symbol(Symbol::Add)),
        };
        let left = Grid::new()
            .columns([
                GridLength::Pixel(56.0),
                GridLength::Star(1.0),
                GridLength::Auto,
            ])
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0)
            .children((
                art_view,
                StackPanel::new()
                    .margin(12.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(1)
                    .children((
                        TextBlock::new()
                            .text(title)
                            .font_size(14.0)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .text_wrapping(TextWrapping::NoWrap)
                            .max_lines(1)
                            .text_trimming(TextTrimming::CharacterEllipsis),
                        artist_view,
                    )),
                like_button,
            ));

        // --- Center: controls row + progress row, stacked. ---
        let shuffle_on = self.settings.shuffle;
        let play_button = Border::new()
            .width(40.0)
            .height(40.0)
            .corner_radius(CornerRadius::uniform(20.0))
            .background(ThemeBrush::Accent)
            .opacity(if has_current { 1.0 } else { 0.4 })
            .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                if has_current && info.is_left_button_pressed {
                    Message::PlayPause
                } else {
                    Message::Noop
                }
            }))
            .content(
                SymbolIcon::new()
                    .symbol(if self.playing {
                        Symbol::Pause
                    } else {
                        Symbol::Play
                    })
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center),
            );
        let repeat_icon = match self.settings.repeat {
            RepeatMode::One => Symbol::RepeatOne,
            _ => Symbol::RepeatAll,
        };
        let repeat_on = self.settings.repeat != RepeatMode::Off;
        let center = StackPanel::new()
            .spacing(2.0)
            .vertical_alignment(VerticalAlignment::Center)
            .margin(8.0)
            .grid_column(1)
            .children((
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(8.0)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .children((
                        Button::new()
                            .is_enabled(has_current)
                            .opacity(if shuffle_on { 1.0 } else { 0.5 })
                            .on_click(context.message(Message::ToggleShuffle))
                            .content(SymbolIcon::new().symbol(Symbol::Shuffle)),
                        Button::new()
                            .is_enabled(has_current)
                            .on_click(context.message(Message::Previous))
                            .content(SymbolIcon::new().symbol(Symbol::Previous)),
                        play_button,
                        Button::new()
                            .is_enabled(has_current)
                            .on_click(context.message(Message::Next))
                            .content(SymbolIcon::new().symbol(Symbol::Next)),
                        Button::new()
                            .is_enabled(has_current)
                            .opacity(if repeat_on { 1.0 } else { 0.5 })
                            .on_click(context.message(Message::CycleRepeat))
                            .content(SymbolIcon::new().symbol(repeat_icon)),
                    )),
                Grid::new()
                    .columns([GridLength::Auto, GridLength::Star(1.0), GridLength::Auto])
                    .children((
                        TextBlock::new()
                            .text(views::fmt_time(self.position))
                            .font_size(12.0)
                            .opacity(0.7)
                            .vertical_alignment(VerticalAlignment::Center)
                            .grid_column(0),
                        Slider::new()
                            .minimum(0.0)
                            .maximum(self.duration.max(1.0))
                            .value(self.position.min(self.duration.max(1.0)))
                            .grid_column(1)
                            .on_value_changed(context.callback(|value: f64| Message::Seek(value))),
                        TextBlock::new()
                            .text(views::fmt_time(self.duration))
                            .font_size(12.0)
                            .opacity(0.7)
                            .vertical_alignment(VerticalAlignment::Center)
                            .grid_column(2),
                    )),
            ));

        // --- Right: lyrics, queue, mute + volume. ---
        let lyrics_button: View = match &current {
            Some(song) => Button::new()
                .on_click(context.message(Message::OpenDetails(DetailsTarget {
                    kind: "song".to_string(),
                    id: song.id.clone(),
                    title: song.name.clone(),
                    subtitle: views::artist_names(song),
                    art: views::art_url(&song.image, "500x500"),
                })))
                .content(SymbolIcon::new().symbol(Symbol::MusicInfo)),
            None => Button::new()
                .is_enabled(false)
                .content(SymbolIcon::new().symbol(Symbol::MusicInfo)),
        };
        let right = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(4.0)
            .vertical_alignment(VerticalAlignment::Center)
            .margin(8.0)
            .grid_column(2)
            .children((
                Button::new()
                    .on_click(context.message(Message::ToggleNowPlaying))
                    .content("Now playing"),
                lyrics_button,
                Button::new()
                    .on_click(context.message(Message::OpenQueue))
                    .content(SymbolIcon::new().symbol(Symbol::List)),
                Button::new()
                    .on_click(context.message(Message::ToggleMute))
                    .content(SymbolIcon::new().symbol(if self.muted {
                        Symbol::Mute
                    } else {
                        Symbol::Volume
                    })),
                Slider::new()
                    .minimum(0.0)
                    .maximum(1.0)
                    // Step frequency defaults to 1.0 in WinUI, which
                    // snapped this 0-1 slider to 0 or 1 only.
                    .step_frequency(0.01)
                    .value(if self.muted {
                        0.0
                    } else {
                        self.settings.volume
                    })
                    .width(100.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .on_value_changed(context.callback(|value: f64| Message::VolumeChanged(value))),
            ));

        Grid::new()
            .columns([
                GridLength::Pixel(320.0),
                GridLength::Star(1.0),
                GridLength::Auto,
            ])
            .margin(8.0)
            .grid_row(2)
            .children((left, center, right))
    }

    /// The Now Playing sidebar: a 340px panel on the right of the
    /// content row, shown when media starts playing. Top to bottom:
    /// a Next-in-queue card (with a button for the full queue), then
    /// the now-playing details — big art, title, clickable artists,
    /// album, Like. The close button and the player bar's Now playing
    /// toggle hide it; the next track start opens it again.
    /// The Now Playing sidebar: a 340px panel on the right of the
    /// content row, shown when media starts playing. Opaque (a solid
    /// background — the old card fill let the page show through).
    /// Top to bottom: the current song's details, the main artist's
    /// brief [picture, details], then the next song in the queue
    /// with the Show full queue button (which opens the queue
    /// overlay over the whole sidebar). The close button and the
    /// player bar's Now playing toggle hide it; the next track start
    /// opens it again.
    fn now_playing_sidebar(&self, context: &mut ViewContext<Self>) -> View {
        let mut rows: Vec<KeyedView> = vec![KeyedView::new(
            "header",
            Grid::new()
                .columns([GridLength::Star(1.0), GridLength::Auto])
                .children((
                    TextBlock::new()
                        .text("Now playing")
                        .font_size(20.0)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .vertical_alignment(VerticalAlignment::Center)
                        .grid_column(0),
                    Button::new()
                        .grid_column(1)
                        .on_click(context.message(Message::ToggleNowPlaying))
                        .content("×"),
                )),
        )];

        match self.player.current() {
            Some(song) => {
                // 1. Current song details, on top.
                let art: View = match views::art_url(&song.image, "500x500") {
                    Some(url) => Border::new()
                        .width(308.0)
                        .height(308.0)
                        .corner_radius(CornerRadius::uniform(8.0))
                        .content(
                            views::image_from_url(&url)
                                .width(308.0)
                                .height(308.0)
                                .stretch(Stretch::UniformToFill),
                        ),
                    None => Border::new()
                        .width(308.0)
                        .height(308.0)
                        .corner_radius(CornerRadius::uniform(8.0))
                        .background(Brush::Theme(ThemeBrush::SolidBackground))
                        .content(
                            SymbolIcon::new()
                                .symbol(Symbol::Audio)
                                .horizontal_alignment(HorizontalAlignment::Center)
                                .vertical_alignment(VerticalAlignment::Center),
                        ),
                };
                rows.push(KeyedView::new("art", art));
                rows.push(KeyedView::new(
                    "title",
                    TextBlock::new()
                        .text(song.name.clone())
                        .font_size(20.0)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .text_wrapping(TextWrapping::Wrap)
                        .max_lines(2),
                ));
                rows.push(KeyedView::new(
                    "artists",
                    views::artist_links(&song.artists, 2, 44, context),
                ));
                match (&song.album.id, &song.album.name) {
                    (Some(id), Some(name)) => {
                        let target = DetailsTarget {
                            kind: "album".to_string(),
                            id: id.clone(),
                            title: name.clone(),
                            subtitle: String::new(),
                            art: None,
                        };
                        rows.push(KeyedView::new(
                            "album",
                            Border::new()
                                .background(Brush::Solid(Color::transparent()))
                                .on_pointer_pressed(context.callback(
                                    move |_info: PointerEventInfo| {
                                        Message::OpenDetails(target.clone())
                                    },
                                ))
                                .content(
                                    TextBlock::new()
                                        .text(name.clone())
                                        .font_size(14.0)
                                        .opacity(0.8),
                                ),
                        ));
                    }
                    (None, Some(name)) => {
                        rows.push(KeyedView::new(
                            "album",
                            TextBlock::new()
                                .text(name.clone())
                                .font_size(14.0)
                                .opacity(0.8),
                        ));
                    }
                    _ => {}
                }
                let liked = self.library.is_liked(&song.id);
                rows.push(KeyedView::new(
                    "like",
                    Button::new()
                        .on_click(context.message(Message::ToggleLike(song.clone())))
                        .content(
                            StackPanel::new()
                                .orientation(Orientation::Horizontal)
                                .spacing(8.0)
                                .children((
                                    SymbolIcon::new().symbol(if liked {
                                        Symbol::Favorite
                                    } else {
                                        Symbol::Add
                                    }),
                                    TextBlock::new()
                                        .text(if liked { "Liked" } else { "Like" })
                                        .font_size(14.0),
                                )),
                        ),
                ));

                // 2. Main artist brief: [picture, details].
                if let Some(artist) = song.artists.primary.first() {
                    let target = DetailsTarget {
                        kind: "artist".to_string(),
                        id: artist.id.clone(),
                        title: artist.name.clone(),
                        subtitle: String::new(),
                        art: views::art_url(&artist.image, "150x150"),
                    };
                    // Picture: circular, from the fetched brief when
                    // it has loaded, else the song's artist image.
                    let art_url = self
                        .now_playing_artist
                        .as_ref()
                        .and_then(|a| views::art_url(&a.image, "150x150"))
                        .or_else(|| views::art_url(&artist.image, "150x150"));
                    let picture: View = match art_url {
                        Some(url) => Border::new()
                            .width(64.0)
                            .height(64.0)
                            .corner_radius(CornerRadius::uniform(32.0))
                            .content(
                                views::image_from_url(&url)
                                    .width(64.0)
                                    .height(64.0)
                                    .stretch(Stretch::UniformToFill),
                            ),
                        None => Border::new()
                            .width(64.0)
                            .height(64.0)
                            .corner_radius(CornerRadius::uniform(32.0))
                            .background(Brush::Theme(ThemeBrush::CardBackground))
                            .content(
                                SymbolIcon::new()
                                    .symbol(Symbol::Audio)
                                    .horizontal_alignment(HorizontalAlignment::Center)
                                    .vertical_alignment(VerticalAlignment::Center),
                            ),
                    };
                    // Details: name + followers once the brief loads.
                    let detail: View = match self
                        .now_playing_artist
                        .as_ref()
                        .and_then(|a| a.follower_count)
                    {
                        Some(count) => StackPanel::new()
                            .vertical_alignment(VerticalAlignment::Center)
                            .spacing(4.0)
                            .children((
                                TextBlock::new()
                                    .text(artist.name.clone())
                                    .font_size(14.0)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .text_wrapping(TextWrapping::Wrap),
                                TextBlock::new()
                                    .text(format!("{count} followers"))
                                    .font_size(12.0)
                                    .opacity(0.7)
                                    .text_wrapping(TextWrapping::Wrap),
                            )),
                        None => TextBlock::new()
                            .text(artist.name.clone())
                            .font_size(14.0)
                            .font_weight(FontWeight::SEMI_BOLD)
                            .vertical_alignment(VerticalAlignment::Center)
                            .text_wrapping(TextWrapping::Wrap)
                            .into(),
                    };
                    rows.push(KeyedView::new(
                        "artistlabel",
                        TextBlock::new()
                            .text("Artist")
                            .font_size(14.0)
                            .font_weight(FontWeight::SEMI_BOLD),
                    ));
                    rows.push(KeyedView::new(
                        "artist",
                        Border::new()
                            .background(Brush::Solid(Color::transparent()))
                            .on_pointer_pressed(context.callback(move |_info: PointerEventInfo| {
                                Message::OpenDetails(target.clone())
                            }))
                            .content(
                                StackPanel::new()
                                    .orientation(Orientation::Horizontal)
                                    .spacing(12.0)
                                    .children((picture, detail)),
                            ),
                    ));
                }

                // 3. Next in the queue + Show full queue.
                rows.push(KeyedView::new(
                    "nextlabel",
                    TextBlock::new()
                        .text("Next in the queue")
                        .font_size(14.0)
                        .font_weight(FontWeight::SEMI_BOLD),
                ));
                let next_index = self.player.index.map(|i| i + 1);
                let next_song = next_index.and_then(|i| self.player.queue.get(i));
                match next_song {
                    Some(next) => {
                        let queue = self.player.queue.clone();
                        let idx = next_index.unwrap_or(0);
                        let art: View = match views::art_url(&next.image, "150x150") {
                            Some(url) => views::image_from_url(&url)
                                .width(48.0)
                                .height(48.0)
                                .stretch(Stretch::UniformToFill)
                                .into(),
                            None => Border::new().width(48.0).height(48.0).content(
                                SymbolIcon::new()
                                    .symbol(Symbol::Audio)
                                    .horizontal_alignment(HorizontalAlignment::Center)
                                    .vertical_alignment(VerticalAlignment::Center),
                            ),
                        };
                        rows.push(KeyedView::new(
                            "next",
                            Border::new()
                                .background(Brush::Theme(ThemeBrush::CardBackground))
                                .border_brush(Brush::Theme(ThemeBrush::CardStroke))
                                .border_thickness(1.0)
                                .corner_radius(CornerRadius::uniform(8.0))
                                .padding(12.0)
                                .on_pointer_pressed(context.callback(
                                    move |_info: PointerEventInfo| {
                                        Message::PlaySong(queue.clone(), idx)
                                    },
                                ))
                                .content(
                                    StackPanel::new()
                                        .orientation(Orientation::Horizontal)
                                        .spacing(12.0)
                                        .children((
                                            art,
                                            StackPanel::new()
                                                .vertical_alignment(VerticalAlignment::Center)
                                                .children((
                                                    TextBlock::new()
                                                        .text(next.name.clone())
                                                        .font_size(14.0)
                                                        .font_weight(FontWeight::SEMI_BOLD)
                                                        .text_wrapping(TextWrapping::Wrap),
                                                    TextBlock::new()
                                                        .text(views::artist_names(next))
                                                        .font_size(12.0)
                                                        .opacity(0.7)
                                                        .text_wrapping(TextWrapping::Wrap),
                                                )),
                                        )),
                                ),
                        ));
                    }
                    None => {
                        rows.push(KeyedView::new(
                            "nextempty",
                            TextBlock::new()
                                .text("Nothing next in the queue.")
                                .font_size(14.0)
                                .opacity(0.7),
                        ));
                    }
                }
                rows.push(KeyedView::new(
                    "fullqueue",
                    Button::new()
                        .on_click(context.message(Message::OpenQueueOverlay))
                        .content("Show full queue"),
                ));
            }
            None => {
                rows.push(KeyedView::new(
                    "empty",
                    TextBlock::new()
                        .text("Nothing playing yet — play a song to see it here.")
                        .font_size(14.0)
                        .opacity(0.7)
                        .text_wrapping(TextWrapping::Wrap),
                ));
            }
        }

        Border::new()
            .grid_row(1)
            .width(340.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Stretch)
            // Opaque: a solid fill, so the page behind never bleeds
            // through the sidebar (CardBackground is translucent
            // over Mica).
            .background(Brush::Theme(ThemeBrush::SolidBackground))
            .border_brush(Brush::Theme(ThemeBrush::CardStroke))
            .border_thickness(1.0)
            .content(
                ScrollViewer::new().content(
                    StackPanel::new()
                        .margin(16.0)
                        .spacing(16.0)
                        .keyed_children(rows),
                ),
            )
    }

    /// The full-queue panel that opens over the Now Playing sidebar
    /// (from its Show full queue button). A 340px panel in the same
    /// cell as the sidebar, painted above it: header (Queue + a
    /// close button) and the queue's song rows. It rises in and
    /// drops out with a fade + a slight scale — Reactor 0.100 has
    /// no translate animation, so this is the smooth motion
    /// available for a bottom-up panel.
    fn queue_overlay(&self, context: &mut ViewContext<Self>) -> View {
        let queue = self.player.queue.clone();
        let header = Grid::new()
            .grid_row(0)
            .columns([GridLength::Star(1.0), GridLength::Auto])
            .children((
                TextBlock::new()
                    .text("Queue")
                    .font_size(20.0)
                    .font_weight(FontWeight::SEMI_BOLD)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(0),
                Button::new()
                    .grid_column(1)
                    .on_click(context.message(Message::CloseQueueOverlay))
                    .content("×"),
            ));
        let mut song_rows: Vec<KeyedView> = Vec::new();
        if queue.is_empty() {
            song_rows.push(KeyedView::new(
                "empty",
                TextBlock::new()
                    .text("Queue is empty.")
                    .font_size(14.0)
                    .opacity(0.7),
            ));
        } else {
            for (i, song) in queue.iter().enumerate() {
                song_rows.push(KeyedView::new(
                    format!("{i}-{}", song.id),
                    views::song_row(song, &queue, self, context),
                ));
            }
        }
        Border::new()
            .grid_row(1)
            .width(340.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Stretch)
            .background(Brush::Theme(ThemeBrush::SolidBackground))
            .border_brush(Brush::Theme(ThemeBrush::CardStroke))
            .border_thickness(1.0)
            .opacity(if self.queue_overlay_shown { 1.0 } else { 0.0 })
            .opacity_transition(std::time::Duration::from_millis(300))
            .scale(if self.queue_overlay_shown { 1.0 } else { 0.96 })
            .scale_transition(std::time::Duration::from_millis(300))
            .content(
                Grid::new()
                    .margin(16.0)
                    .rows([GridLength::Auto, GridLength::Star(1.0)])
                    .children((
                        header,
                        ScrollViewer::new()
                            .grid_row(1)
                            .margin(Thickness::new(0.0, 16.0, 0.0, 0.0))
                            .content(StackPanel::new().spacing(8.0).keyed_children(song_rows)),
                    )),
            )
    }
}
