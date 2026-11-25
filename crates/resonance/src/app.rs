//! The app shell: NavigationView sidebar (search box is the Search
//! nav row), page content, and the bottom player bar. There is no
//! custom title bar — the page fills the full client height and the
//! window uses the standard system title bar.
//!
//! WinUI design notes (Windows app design guidelines):
//! - NavigationView in Left mode is the shell's primary navigation.
//! - The sidebar search box is the app's primary command: it is the
//!   Search nav row, with live autocomplete suggestions.
//! - The bottom player bar is a persistent 96px media bar modeled on
//!   Spotify's Now Playing bar: track info left, controls + progress
//!   center, extras (lyrics, queue, volume) right.
//! - Spacing is 4/8/12/24; type ramps 12/14/20/28.
//!
//! windows-reactor 0.100 API notes (verified against the crate):
//! - Layout props (`margin`, `grid_row`, `grid_column`, `width`)
//!   are set on the builder *before* the terminal `.content()` /
//!   `.children()` / `.slots()` call, which returns a `View`.
//! - Multi-slot controls (NavigationView, NavigationViewItem)
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
    PaneOpenChanged(bool),
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

/// The Search entry in the sidebar is a search box, not a plain
/// nav row: an AutoSuggestBox with live suggestions. Clicking it
/// selects the Search row (which navigates to the Search page) and
/// typing searches live, exactly as the old title-bar box did. The
/// input lives here now — not in a title bar, not on the Search
/// page.
fn search_nav_item(shell: &Shell, context: &mut ViewContext<Shell>) -> KeyedView {
    KeyedView::new(
        "search".to_string(),
        NavigationViewItem::new()
            .tag("search")
            .is_selected(shell.page == Page::Search)
            .selects_on_invoked(true)
            .slots([
                SlotView::new(
                    NavigationViewItemSlot::Content,
                    AutoSuggestBox::new()
                        .text(shell.search_text.clone())
                        .placeholder_text("Search songs, albums, artists...")
                        .horizontal_alignment(HorizontalAlignment::Stretch)
                        .items_source(shell.search_suggestions.clone())
                        .on_text_changed(
                            context.callback(|value: String| Message::SearchTextChanged(value)),
                        )
                        .on_suggestion_chosen(
                            context.callback(|value: String| Message::SuggestionChosen(value)),
                        ),
                ),
                SlotView::new(
                    NavigationViewItemSlot::Icon,
                    SymbolIcon::new().symbol(Symbol::Find),
                ),
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

        // Home data loads in the background as the shell opens.
        spawn_home_load(context);

        Self {
            page: Page::Home,
            details: None,
            back_page: Page::Home,
            hovered_card: None,
            menu: None,
            properties: None,
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
            Message::PaneOpenChanged(open) => self.pane_open = open,
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
                    Ok(data) => self.home = Some(data),
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
                        }
                        Err(err) => self.search_error = Some(err),
                    }
                }
            }
            Message::PlaySong(queue, index) => {
                let quality = Quality::from_setting(self.settings.quality);
                self.player.load_queue(queue, index, quality);
                if let Some(song) = self.player.current() {
                    self.library.push_recent(song.clone());
                }
                self.playing = true;
                self.position = 0.0;
                self.duration = self.player.duration_secs();
                spawn_tick(context);
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
                // drill-in included.
                self.details = None;
                self.page = Page::Queue;
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
                        Ok(content) => details.content = Some(content),
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

        // No custom title bar: the page (sidebar + content) fills
        // the full client height, and the window uses the standard
        // system title bar. The drill-in back button lives in the
        // NavigationView header instead (see below); the theme
        // switcher lives in Settings.
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

        // The drill-in back button lives in the NavigationView
        // header, above the page content (the title bar is gone).
        // No drill-in, no button — an empty header takes no space.
        let back_button: View = if self.details.is_some() || self.page == Page::Queue {
            Button::new()
                .margin(Thickness::new(24.0, 12.0, 0.0, 4.0))
                .on_click(context.message(Message::Back))
                .content("‹ Back")
        } else {
            TextBlock::new().text("").into()
        };

        let navigation = NavigationView::new()
            .pane_display_mode(NavigationViewPaneDisplayMode::Left)
            .is_pane_toggle_button_visible(true)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .is_settings_visible(false)
            .always_show_header(true)
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
            .grid_row(0)
            .slots([
                SlotView::new(NavigationViewSlot::Header, back_button),
                SlotView::collection(
                    NavigationViewSlot::MenuItems,
                    vec![
                        nav_item("home", "Home", Symbol::Home, self.page == Page::Home),
                        search_nav_item(self, context),
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

        let shell_grid = Grid::new()
            .rows([GridLength::Star(1.0), GridLength::Pixel(96.0)])
            .children((navigation, player_bar));

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
        context.spawn_background(move |_| {
            let result =
                data::runtime_handle().block_on(async { data::client().search_all(&query).await });
            Message::SearchAllLoaded(query, result.map_err(|err| err.to_string()))
        });
    }

    /// Shared bookkeeping after Next/Previous/auto-advance.
    fn after_track_change(&mut self, context: &ComponentContext<Self>) {
        if let Some(song) = self.player.current() {
            self.library.push_recent(song.clone());
        }
        self.playing = self.player.is_playing();
        self.position = self.player.position_secs();
        self.duration = self.player.duration_secs();
        if self.player.has_current() {
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
            .grid_row(1)
            .children((left, center, right))
    }
}
