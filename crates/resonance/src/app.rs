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

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use saavnapi::{Album, Artist, ArtistArgs, LaunchData, Playlist, SearchAll, Song};
use windows_reactor::*;

use crate::data;
use crate::pages;
use crate::player::{Player, Quality, SmtcCommand};
use crate::store::{Library, Material, PinnedItem, QualitySetting, RepeatMode, Settings, Theme};
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
    /// A per-push sequence number: it tags the background load, so
    /// a late reply lands on the entry it was fetched for (not on
    /// a different entry for the same target pushed later).
    pub seq: u64,
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
    /// The details drill-in stack: an inner route history for the
    /// current page. Each new details push adds an entry; Back pops
    /// one, so Home → album → album → artist comes back in reverse
    /// order instead of jumping straight to the page. The top entry
    /// is what renders. Cleared when the top-level page changes,
    /// a search starts, or the Queue page opens.
    pub details: Vec<DetailsState>,
    /// Next sequence number for a pushed details entry.
    pub details_seq: u64,
    /// Page the title-bar back button returns to (Details, Queue).
    pub back_page: Page,
    /// Card id the pointer is over (its hover buttons show).
    pub hovered_card: Option<String>,
    /// Song id whose like button the pointer is over (its icon goes
    /// to full contrast; otherwise it is greyish).
    pub hovered_like: Option<String>,
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
    pub pane_open: bool,
    pub search_text: String,
    /// A counter bumped on every keystroke and every suggestion
    /// pick; a debounced search only fires when its seq still
    /// matches, so a burst of typing sends one `search_all`.
    pub search_debounce_seq: u64,
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
    /// The SMTC button channel's receiving end, shared with the
    /// listener the Shell spawns (one listener at a time, blocking
    /// on recv; the Smtc handler re-spawns it after each press).
    /// None when the player has no media session (no audio device).
    pub smtc_rx: Option<Arc<Mutex<Receiver<SmtcCommand>>>>,
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
    /// Fired ~300ms after a keystroke, once typing has paused. The
    /// seq is the `search_debounce_seq` at the time the timer was
    /// started; the handler drops it when a newer keystroke or a
    /// suggestion pick superseded it.
    SearchDebounced(u64, String),
    SuggestionChosen(String),
    HomeLoaded(Result<LaunchData, String>),
    SearchAllLoaded(String, Result<SearchAll, String>),
    PlaySong(Vec<Song>, usize),
    ShufflePlay(Vec<Song>),
    PlayPause,
    Next,
    Previous,
    Seek(f64),
    VolumeChanged(f64),
    Tick,
    ToggleLike(Song),
    ToggleLikeAlbum(Album),
    ToggleLikePlaylist(Playlist),
    ToggleLikeArtist(Artist),
    PlayByKind(String, String),
    OpenDetails(DetailsTarget),
    DetailsLoaded(u64, Result<DetailsContent, String>),
    CardHover(Option<String>),
    LikeHover(Option<String>),
    CardPressed(DetailsTarget, PointerEventInfo),
    OpenMenu(DetailsTarget, f64, f64),
    CloseOverlay,
    MenuPlay(DetailsTarget),
    MenuPlayNext(DetailsTarget),
    MenuProperties(DetailsTarget),
    /// Pin or unpin a media item (from the card context menu).
    TogglePin(DetailsTarget),
    /// Play a pinned song by id (its sidebar row only stores the id,
    /// so the full Song is fetched first).
    PlayPinnedSong(String),
    PlayNext(Vec<Song>),
    PropertiesLoaded(Result<DetailsContent, String>),
    CloseProperties,
    ToggleNowPlaying,
    /// A button press from the OS media session (SMTC). The handler
    /// re-spawns the listener first, then routes the press through
    /// the same PlayPause / Next / Previous handlers the UI uses.
    Smtc(SmtcCommand),
    ArtistBriefLoaded(String, Result<Artist, String>),
    OpenQueueOverlay,
    CloseQueueOverlay,
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

/// Fetch one pinned song by id and play it. A pinned song's sidebar
/// row only stores the id, so the full Song (with its stream URLs)
/// is fetched on click. Failures are silent (Noop).
fn spawn_play_pinned_song(context: &ComponentContext<Shell>, id: String) {
    context.spawn_background(move |_| {
        let result = data::runtime_handle()
            .block_on(async { data::client().get_songs_by_ids(&[id.as_str()]).await });
        match result {
            Ok(songs) if !songs.is_empty() => Message::PlaySong(songs, 0),
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
fn spawn_details_load(context: &ComponentContext<Shell>, seq: u64, target: DetailsTarget) {
    context.spawn_background(move |_| {
        let result =
            data::runtime_handle().block_on(async { load_details(&data::client(), &target).await });
        Message::DetailsLoaded(seq, result.map_err(|err| err.to_string()))
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

/// Wait ~300ms after the last keystroke, then fire the search — a
/// burst of typing sends one `search_all`, not one per keystroke.
/// The seq lets the handler drop a timer a newer keystroke
/// superseded.
fn spawn_search_debounce(context: &ComponentContext<Shell>, seq: u64, query: String) {
    context.spawn_background(move |_| {
        std::thread::sleep(Duration::from_millis(300));
        Message::SearchDebounced(seq, query)
    });
}

/// A borderless icon button for the player bar: a transparent
/// Border with a centered SymbolIcon (no WinUI Button chrome — a
/// Button has no background setter in 0.100, so its border can't
/// be removed), a tooltip, and a pointer-press handler. `enabled`
/// gates the press; the icon dims to 0.4 when it is false.
fn bar_icon_button(
    context: &mut ViewContext<Shell>,
    icon: Symbol,
    tooltip: &str,
    opacity: f64,
    enabled: bool,
    message: Message,
) -> View {
    Border::new()
        .width(32.0)
        .height(32.0)
        .background(Brush::Solid(Color::transparent()))
        .opacity(opacity)
        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
            if enabled && info.is_left_button_pressed {
                message.clone()
            } else {
                Message::Noop
            }
        }))
        .content(
            SymbolIcon::new()
                .symbol(icon)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .tooltip(tooltip)
}

/// A top-level nav item (Home/Search/Library). The label is a
/// transparent Border with a pointer handler, not a plain string, so
/// tapping the item always navigates — even when it is already
/// selected. A NavigationView only fires its selection callback when
/// the selection CHANGES, so tapping the already-selected item used
/// to do nothing, leaving the user stuck deep in a details stack
/// with only the back button to climb out. The Border press sends
/// Navigate, which returns to the section's first page and pops
/// every detail layer of that journey. (The selection callback
/// still fires too when the item was not selected; both send the
/// same Navigate, which is idempotent.)
fn nav_item(
    tag: &str,
    label: &str,
    icon: Symbol,
    selected: bool,
    page: Page,
    context: &mut ViewContext<Shell>,
) -> KeyedView {
    KeyedView::new(
        tag.to_string(),
        NavigationViewItem::new()
            .tag(tag)
            .is_selected(selected)
            .selects_on_invoked(true)
            .slots([
                SlotView::new(
                    NavigationViewItemSlot::Content,
                    Border::new()
                        .background(Brush::Solid(Color::transparent()))
                        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                            if info.is_left_button_pressed {
                                Message::Navigate(page)
                            } else {
                                Message::Noop
                            }
                        }))
                        .content(TextBlock::new().text(label).font_size(14.0)),
                ),
                SlotView::new(NavigationViewItemSlot::Icon, SymbolIcon::new().symbol(icon)),
            ]),
    )
}

/// The divider after Library: a 1px line in a nav-item row. It is a
/// NavigationViewItem (Reactor 0.100 has no NavigationViewItemSeparator)
/// with `selects_on_invoked(false)`, so clicking it never selects
/// anything. The line runs end to end, left to right — no horizontal
/// margin, only vertical spacing. Shown only while pinned items exist.
fn nav_divider() -> KeyedView {
    KeyedView::new(
        "pinned-divider".to_string(),
        NavigationViewItem::new()
            .selects_on_invoked(false)
            .slots([SlotView::new(
                NavigationViewItemSlot::Content,
                Border::new()
                    .height(1.0)
                    .horizontal_alignment(HorizontalAlignment::Stretch)
                    .margin(Thickness::new(0.0, 8.0, 0.0, 8.0))
                    .background(Brush::Theme(ThemeBrush::CardStroke)),
            )]),
    )
}

/// One pinned item as a nav row: 48px art (the same size as the
/// song-row art in the details views) + title only (no subtitle),
/// in a Border so the whole row is clickable. The 48px art makes
/// the row taller than a plain nav item. A pinned item is a shortcut,
/// not a page — `selects_on_invoked(false)` so clicking it never
/// changes the nav selection; the Border's pointer press opens/plays
/// it instead. A pinned song plays on click; any other kind opens
/// its details page. On hover, a circular play icon appears over the
/// art; clicking it plays the item — a song plays itself, an artist
/// plays their top songs, an album/playlist/chart plays its songs.
/// The play icon is a sibling of the row (not a child), so its press
/// never also triggers the row's press handler.
fn pinned_nav_item(
    pinned: &PinnedItem,
    hovered: bool,
    context: &mut ViewContext<Shell>,
) -> KeyedView {
    let item = pinned.clone();
    let message = if item.kind == "song" {
        Message::PlayPinnedSong(item.id.clone())
    } else {
        Message::OpenDetails(DetailsTarget {
            kind: item.kind.clone(),
            id: item.id.clone(),
            title: item.title.clone(),
            subtitle: item.subtitle.clone(),
            art: item.art.clone(),
        })
    };
    // The hover play icon plays the item: a song plays itself, an
    // artist plays their top songs, anything else plays its songs.
    let play_message = if item.kind == "song" {
        Message::PlayPinnedSong(item.id.clone())
    } else {
        Message::PlayByKind(item.kind.clone(), item.id.clone())
    };
    // Artist art is a full circle; every other kind is a 4px
    // rounded square.
    let art_radius = if item.kind == "artist" { 24.0 } else { 4.0 };
    let art_border: View = match &item.art {
        Some(url) => Border::new()
            .width(48.0)
            .height(48.0)
            .corner_radius(CornerRadius::uniform(art_radius))
            .content(
                views::image_from_url(url)
                    .width(48.0)
                    .height(48.0)
                    .stretch(Stretch::UniformToFill),
            ),
        None => Border::new()
            .width(48.0)
            .height(48.0)
            .corner_radius(CornerRadius::uniform(art_radius))
            .background(Brush::Theme(ThemeBrush::CardBackground))
            .content(
                SymbolIcon::new()
                    .symbol(Symbol::Audio)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center),
            ),
    };
    let title = TextBlock::new()
        .text(item.title.clone())
        .font_size(14.0)
        .text_wrapping(TextWrapping::NoWrap)
        .text_trimming(TextTrimming::CharacterEllipsis)
        .max_lines(1);
    let subtitle = TextBlock::new()
        .text(item.subtitle.clone())
        .font_size(12.0)
        .opacity(0.7)
        .text_wrapping(TextWrapping::NoWrap)
        .text_trimming(TextTrimming::CharacterEllipsis)
        .max_lines(1);
    // Title above subtitle, with a small gap between them; the
    // subtitle is only shown when the item has one.
    let mut text_children: Vec<KeyedView> = vec![KeyedView::new("title", title)];
    if !item.subtitle.is_empty() {
        text_children.push(KeyedView::new("subtitle", subtitle));
    }
    let text_column = StackPanel::new()
        .orientation(Orientation::Vertical)
        .spacing(2.0)
        .vertical_alignment(VerticalAlignment::Center)
        .keyed_children(text_children);
    let row = Border::new()
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .background(Brush::Solid(Color::transparent()))
        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
            if info.is_left_button_pressed {
                message.clone()
            } else {
                Message::Noop
            }
        }))
        .content(
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .vertical_alignment(VerticalAlignment::Center)
                .children((art_border, text_column)),
        );
    let mut content_children: Vec<KeyedView> = vec![KeyedView::new("row", row)];
    if hovered {
        content_children.push(KeyedView::new(
            "play",
            Border::new()
                .width(32.0)
                .height(32.0)
                .corner_radius(CornerRadius::uniform(16.0))
                .border_thickness(1.0)
                .border_brush(Brush::Theme(ThemeBrush::CardStroke))
                .background(Brush::Theme(ThemeBrush::CardBackground))
                .horizontal_alignment(HorizontalAlignment::Left)
                .vertical_alignment(VerticalAlignment::Center)
                .margin(Thickness::new(8.0, 0.0, 0.0, 0.0))
                .on_pointer_pressed(
                    context.callback(move |_info: PointerEventInfo| play_message.clone()),
                )
                .content(SymbolIcon::new().symbol(Symbol::Play)),
        ));
    }
    let hover_id = item.id.clone();
    let content =
        Border::new()
            .background(Brush::Solid(Color::transparent()))
            // A little vertical breathing room between pinned rows.
            .margin(Thickness::new(0.0, 3.0, 0.0, 3.0))
            .on_pointer_entered(context.callback(move |_info: PointerEventInfo| {
                Message::CardHover(Some(hover_id.clone()))
            }))
            .on_pointer_exited(context.callback(|_info: PointerEventInfo| Message::CardHover(None)))
            .content(Grid::new().keyed_children(content_children));
    KeyedView::new(
        format!("pin-{}-{}", item.kind, item.id),
        NavigationViewItem::new()
            .selects_on_invoked(false)
            .slots([SlotView::new(NavigationViewItemSlot::Content, content)]),
    )
}

impl Component for Shell {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let settings = Settings::load();
        let library = Library::load();
        let mut player = Player::new();
        player.set_volume(settings.volume);
        player.set_repeat_all(settings.repeat == RepeatMode::All);
        // The media session's button channel: the Shell listens for
        // OS play/pause/next/previous presses on it.
        let smtc_rx = player
            .take_smtc_receiver()
            .map(|rx| Arc::new(Mutex::new(rx)));

        // Home data loads in the background as the shell opens.
        spawn_home_load(context);

        let shell = Self {
            page: Page::Home,
            details: Vec::new(),
            details_seq: 0,
            back_page: Page::Home,
            hovered_card: None,
            hovered_like: None,
            menu: None,
            properties: None,
            now_playing_open: false,
            now_playing_artist: None,
            now_playing_artist_song_id: None,
            queue_overlay_mounted: false,
            pane_open: true,
            search_text: String::new(),
            search_debounce_seq: 0,
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
            smtc_rx,
            position: 0.0,
            duration: 0.0,
            playing: false,
            muted: false,
            pre_mute_volume: 0.8,
        };
        // Start listening for OS media-session button presses.
        shell.spawn_smtc_listener(context);
        shell
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        match message {
            Message::Navigate(page) => {
                self.page = page;
                // Switching top-level pages leaves a details drill-in.
                self.details.clear();
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
                // `search_all` call feeds BOTH the autocomplete
                // suggestions and the full results, so results never
                // wait for a suggestion to be picked. The query rides
                // along so a slow reply can't overwrite results for
                // newer text.
                let query = text.trim().to_string();
                if query.len() >= 2 {
                    // Typing shows search results, not a stale
                    // details drill-in from another page.
                    self.details.clear();
                    if self.page != Page::Search {
                        self.page = Page::Search;
                    }
                    self.search_loading = true;
                    self.search_error = None;
                    // Debounced: the search only fires once typing
                    // has paused for ~300ms, so a burst sends one
                    // call instead of one per keystroke. The seq
                    // lets the timer's handler drop it when a newer
                    // keystroke started a fresh timer.
                    self.search_debounce_seq += 1;
                    let seq = self.search_debounce_seq;
                    spawn_search_debounce(context, seq, query);
                } else {
                    self.search_suggestions.clear();
                    self.search_all = None;
                    self.search_error = None;
                    self.search_loading = false;
                }
            }
            Message::SearchDebounced(seq, query) => {
                // A newer keystroke (or a picked suggestion) started
                // its own timer — this one is stale. A query that no
                // longer matches the box is stale too (the text was
                // edited or cleared while the timer waited).
                if seq == self.search_debounce_seq && query == self.search_text.trim() {
                    context.spawn_background(move |_| {
                        let result = data::runtime_handle()
                            .block_on(async { data::client().search_all(&query).await });
                        Message::SearchAllLoaded(query, result.map_err(|err| err.to_string()))
                    });
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
            Message::ShufflePlay(mut songs) => {
                // Fisher-Yates with the same xorshift the player uses
                // for shuffle_upcoming, seeded from the clock.
                let mut state = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(0x9E37_79B9_7F4A_7C15)
                    | 1;
                let mut n = songs.len();
                while n > 1 {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    let j = (state % n as u64) as usize;
                    n -= 1;
                    songs.swap(n, j);
                }
                self.update(Message::PlaySong(songs, 0), context);
            }
            Message::Smtc(cmd) => {
                // Re-spawn the listener first, so there is always
                // one waiting for the next press, then route the
                // press through the same handlers the UI uses.
                self.spawn_smtc_listener(context);
                match cmd {
                    // The session only shows Play while paused and
                    // Pause while playing, so the toggle is safe.
                    SmtcCommand::Play | SmtcCommand::Pause => {
                        self.update(Message::PlayPause, context);
                    }
                    SmtcCommand::Next => {
                        self.update(Message::Next, context);
                    }
                    SmtcCommand::Previous => {
                        self.update(Message::Previous, context);
                    }
                }
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
                // The session's Next button follows repeat-all (it
                // wraps on the last song when repeat-all is on).
                self.player
                    .set_repeat_all(self.settings.repeat == RepeatMode::All);
            }
            Message::ToggleNowPlaying => {
                self.now_playing_open = !self.now_playing_open;
                if !self.now_playing_open {
                    // The overlay lives over the sidebar; hiding the
                    // sidebar hides it too.
                    self.queue_overlay_mounted = false;
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
                self.queue_overlay_mounted = true;
            }
            Message::CloseQueueOverlay => {
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
                // Keep the media session's status in step with the
                // player — this is what catches a track ending with
                // repeat off and autoplay off, where the player just
                // stops and no play/pause handler runs.
                self.player.sync_smtc_status();
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
            Message::ToggleLikeArtist(artist) => {
                self.library.toggle_like_artist(artist);
            }
            Message::PlayByKind(kind, id) => {
                spawn_play_by_kind(context, kind, id);
            }
            Message::OpenDetails(target) => {
                self.open_details(context, target);
            }
            Message::DetailsLoaded(seq, result) => {
                // The reply belongs to the stack entry it was
                // fetched for (matched by sequence number). If that
                // entry was already popped (the user went back while
                // it loaded), the reply is dropped.
                if let Some(details) = self.details.iter_mut().find(|d| d.seq == seq) {
                    details.loading = false;
                    match result {
                        Ok(content) => {
                            details.content = Some(content);
                        }
                        Err(err) => details.error = Some(err),
                    }
                }
            }
            Message::CardHover(id) => {
                self.hovered_card = id;
            }
            Message::LikeHover(id) => {
                self.hovered_like = id;
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
            Message::TogglePin(target) => {
                self.menu = None;
                let item = PinnedItem {
                    kind: target.kind.clone(),
                    id: target.id.clone(),
                    title: target.title.clone(),
                    subtitle: target.subtitle.clone(),
                    art: target.art.clone(),
                };
                self.library.toggle_pin(item);
            }
            Message::PlayPinnedSong(id) => {
                spawn_play_pinned_song(context, id);
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
                if !self.details.is_empty() {
                    // Pop one inner route; stay on the same page it
                    // was opened from. The stack makes this walk
                    // back through each details page in reverse.
                    self.details.pop();
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

        let theme_label = match self.settings.theme {
            Theme::Dark => "Theme: Dark",
            Theme::Light => "Theme: Light",
            Theme::System => "Theme: System",
        };

        let on_drill_in = !self.details.is_empty() || self.page == Page::Queue;
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

        // The nav list: Home, Search, Library, then — only while
        // pinned items exist — a divider and the pinned rows below
        // it. A pinned row is a shortcut, not a page, so it never
        // changes the nav selection.
        let mut menu_items: Vec<KeyedView> = vec![
            nav_item(
                "home",
                "Home",
                Symbol::Home,
                self.page == Page::Home,
                Page::Home,
                context,
            ),
            nav_item(
                "search",
                "Search",
                Symbol::Find,
                self.page == Page::Search,
                Page::Search,
                context,
            ),
            nav_item(
                "library",
                "Library",
                Symbol::Audio,
                self.page == Page::Library,
                Page::Library,
                context,
            ),
        ];
        if !self.library.pinned.is_empty() {
            menu_items.push(nav_divider());
            for pinned in &self.library.pinned {
                let hovered = self.hovered_card.as_deref() == Some(pinned.id.as_str());
                menu_items.push(pinned_nav_item(pinned, hovered, context));
            }
        }

        // Settings is pinned to the very bottom of the sidebar, in
        // the PaneFooter. It is a real NavigationViewItem — the same
        // component as Home/Search/Library, so it gets the same hover,
        // selection indicator, and icon + label layout. But a footer
        // item never fires the NavigationView's selection callback
        // (the footer is not part of the nav selection model), so
        // clicking it would do nothing on its own. The label is
        // wrapped in a transparent Border with a pointer handler
        // that navigates to Settings, so clicking the label (and the
        // row around it) works. The selection indicator is set
        // declaratively from the current page, so it tracks correctly
        // even though the nav never selects this item.
        // (0.100 has no FooterMenuItems slot — the PaneFooter is the
        // only bottom-pinned slot.)
        //
        // Do NOT wrap the Icon in a Border: the Icon slot expects an
        // IconElement, and a Border there crashed the app at launch
        // with STATUS_STACK_BUFFER_OVERRUN (0xc0000409) — the same
        // signature as the earlier context-menu crash. The icon is a
        // plain SymbolIcon, so only the label area is clickable.
        let settings_footer = NavigationViewItem::new()
            .tag("settings")
            .is_selected(self.page == Page::Settings)
            .selects_on_invoked(true)
            .slots([
                SlotView::new(
                    NavigationViewItemSlot::Icon,
                    SymbolIcon::new().symbol(Symbol::Setting),
                ),
                SlotView::new(
                    NavigationViewItemSlot::Content,
                    Border::new()
                        .background(Brush::Solid(Color::transparent()))
                        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                            if info.is_left_button_pressed {
                                Message::Navigate(Page::Settings)
                            } else {
                                Message::Noop
                            }
                        }))
                        .content(TextBlock::new().text("Settings").font_size(14.0)),
                ),
            ]);

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
                    Some("settings") => Message::Navigate(Page::Settings),
                    // A null or unknown tag means the selection was
                    // cleared — a drill-in page (Queue), which is not
                    // a nav item — and that is not a navigation.
                    // Mapping null to Home used to bounce every
                    // drill-in straight back to Home the moment it
                    // opened, so the click looked like it did nothing.
                    _ => Message::Noop,
                }),
            )
            .grid_row(1)
            .slots([
                SlotView::collection(NavigationViewSlot::MenuItems, menu_items),
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
                // 112px, so the 40px play/pause button plus the
                // progress row below it are never clipped. The bar
                // was 96px and the play button was still being cut —
                // the controls row plus the seek row plus the center
                // column's margins need more than 96px.
                GridLength::Pixel(112.0),
            ])
            .keyed_children(shell_children);

        // The overlay layer (context menu / Properties dialog) is
        // ALWAYS in the tree, as a sibling of the shell in the same
        // root cell, painted above it. It is empty while no menu or
        // dialog is open, so it never intercepts pointer input.
        // Keeping it mounted means opening a menu only adds children
        // inside the overlay — the shell itself is never reparented.
        // (Reparenting the whole shell on every menu open crashed
        // the app with a stack buffer overrun when the More button
        // was clicked.)
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
        // The click-away layer is only mounted while a menu or
        // dialog is open — the overlay itself is always in the tree
        // (see view()), so an always-on dismiss layer would swallow
        // every click on the shell.
        let mut layers: Vec<KeyedView> = vec![];
        if self.menu.is_some() || self.properties.is_some() {
            layers.push(KeyedView::new(
                "dismiss",
                Border::new()
                    .background(Brush::Solid(Color::transparent()))
                    .on_pointer_pressed(
                        context.callback(|_info: PointerEventInfo| Message::CloseOverlay),
                    ),
            ));
        }
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
        let pin_target = target.clone();
        let pin_label = if self.library.is_pinned(&target.kind, &target.id) {
            "Unpin"
        } else {
            "Pin"
        };
        Border::new()
            .width(220.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .vertical_alignment(VerticalAlignment::Top)
            .margin(Thickness::new(menu.x, menu.y, 0.0, 0.0))
            // Opaque: a menu must not let the page show through it.
            .background(Brush::Theme(ThemeBrush::SolidBackground))
            .border_brush(Brush::Theme(ThemeBrush::CardStroke))
            .border_thickness(1.0)
            .corner_radius(CornerRadius::uniform(8.0))
            .padding(4.0)
            .content(StackPanel::new().spacing(2.0).children((
                menu_row("Play", Message::MenuPlay(play_target), context),
                menu_row("Play next", Message::MenuPlayNext(next_target), context),
                menu_row("Properties", Message::MenuProperties(target), context),
                menu_row(pin_label, Message::TogglePin(pin_target), context),
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
            // Opaque: a dialog must not let the page show through it.
            .background(Brush::Theme(ThemeBrush::SolidBackground))
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
        //
        // And it is a STACK, not one slot: every new details from
        // inside a details page pushes a fresh entry, so Back walks
        // back through album → album → artist in reverse order
        // instead of dropping straight to the page. Pushing the
        // same target that is already on top is a no-op (a
        // double-click must not stack two identical pages).
        self.menu = None;
        if self
            .details
            .last()
            .is_some_and(|d| d.target.kind == target.kind && d.target.id == target.id)
        {
            return;
        }
        let seq = self.details_seq;
        self.details_seq += 1;
        self.details.push(DetailsState {
            seq,
            target: target.clone(),
            loading: true,
            error: None,
            content: None,
        });
        spawn_details_load(context, seq, target);
    }

    /// Open the context menu at window coordinates, clamped so the
    /// ~220x150 menu stays inside the 1280x800 window.
    fn open_menu(&mut self, target: DetailsTarget, x: f64, y: f64) {
        // Guard against non-finite coordinates: a NaN/infinite
        // margin corrupts the native layout pass. A bad value falls
        // back to the top-left.
        let x = if x.is_finite() {
            x.clamp(0.0, 1280.0 - 230.0)
        } else {
            0.0
        };
        let y = if y.is_finite() {
            y.clamp(0.0, 800.0 - 180.0)
        } else {
            0.0
        };
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
        // A picked suggestion searches at once — invalidate any
        // keystroke debounce still waiting, so it can't fire after
        // this one.
        self.search_debounce_seq += 1;
        self.page = Page::Search;
        // Choosing a suggestion shows results, not a stale
        // details drill-in.
        self.details.clear();
        self.search_loading = true;
        self.search_error = None;
        context.spawn_background(move |_| {
            let result =
                data::runtime_handle().block_on(async { data::client().search_all(&query).await });
            Message::SearchAllLoaded(query, result.map_err(|err| err.to_string()))
        });
    }

    /// Listen for one OS media-session button press: block on the
    /// SMTC channel (on the thread pool) and return the press as a
    /// message. The Smtc handler re-spawns this after each press,
    /// so there is always exactly one listener waiting. A dropped
    /// channel (no media session) returns Noop and is not
    /// re-spawned.
    fn spawn_smtc_listener(&self, context: &ComponentContext<Self>) {
        let Some(rx) = self.smtc_rx.clone() else {
            return;
        };
        context.spawn_background(move |_| {
            let cmd = rx.lock().ok().and_then(|rx| rx.recv().ok());
            match cmd {
                Some(cmd) => Message::Smtc(cmd),
                None => Message::Noop,
            }
        });
    }

    /// Reset the page transition (fade + slight zoom) and animate it
    /// back in. Called on every navigation event, so switching
    /// pages, drilling in, and going back all move instead of
    /// cutting. The 30ms settle lets the new page mount invisible
    /// before the transition runs (the same pattern as the
    /// per-page fades).
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
        // The Now Playing sidebar never opens on its own: it only
        // opens and closes from the player bar's Now playing button
        // (ToggleNowPlaying). A track starting just refreshes what
        // the sidebar shows, when it is open.
        if self.player.has_current() {
            spawn_tick(context);
        }
        // The session's status follows the track (Playing while it
        // plays, Paused once it stops).
        self.player.sync_smtc_status();
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
            Some(url) => Border::new()
                .width(56.0)
                .height(56.0)
                .corner_radius(CornerRadius::uniform(4.0))
                .grid_column(0)
                .content(
                    views::image_from_url(&url)
                        .width(56.0)
                        .height(56.0)
                        .stretch(Stretch::UniformToFill),
                ),
            None => Border::new()
                .width(56.0)
                .height(56.0)
                .corner_radius(CornerRadius::uniform(4.0))
                .grid_column(0)
                .content(
                    SymbolIcon::new()
                        .symbol(Symbol::Audio)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .vertical_alignment(VerticalAlignment::Center),
                ),
        };
        let like_button: View = views::like_button(current.as_ref(), 2, self, context);
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
            )
            .tooltip(if self.playing { "Pause" } else { "Play" });
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
                        bar_icon_button(
                            context,
                            Symbol::Shuffle,
                            "Shuffle",
                            if !has_current {
                                0.4
                            } else if shuffle_on {
                                1.0
                            } else {
                                0.5
                            },
                            has_current,
                            Message::ToggleShuffle,
                        ),
                        bar_icon_button(
                            context,
                            Symbol::Previous,
                            "Previous",
                            if has_current { 1.0 } else { 0.4 },
                            has_current,
                            Message::Previous,
                        ),
                        play_button,
                        bar_icon_button(
                            context,
                            Symbol::Next,
                            "Next",
                            if has_current { 1.0 } else { 0.4 },
                            has_current,
                            Message::Next,
                        ),
                        bar_icon_button(
                            context,
                            repeat_icon,
                            match self.settings.repeat {
                                RepeatMode::One => "Repeat one",
                                RepeatMode::All => "Repeat all",
                                RepeatMode::Off => "Repeat off",
                            },
                            if !has_current {
                                0.4
                            } else if repeat_on {
                                1.0
                            } else {
                                0.5
                            },
                            has_current,
                            Message::CycleRepeat,
                        ),
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

        // --- Right: now playing, mute + volume. ---
        let right = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(4.0)
            .vertical_alignment(VerticalAlignment::Center)
            .margin(8.0)
            .grid_column(2)
            .children((
                bar_icon_button(
                    context,
                    Symbol::Audio,
                    "Now playing",
                    1.0,
                    true,
                    Message::ToggleNowPlaying,
                ),
                Button::new()
                    .on_click(context.message(Message::ToggleMute))
                    .content(SymbolIcon::new().symbol(if self.muted {
                        Symbol::Mute
                    } else {
                        Symbol::Volume
                    }))
                    .tooltip(if self.muted { "Unmute" } else { "Mute" }),
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
                    .on_value_changed(context.callback(|value: f64| Message::VolumeChanged(value)))
                    .tooltip("Volume"),
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
    /// content row. It only opens and closes from the player bar's
    /// Now playing button (ToggleNowPlaying) — a track starting
    /// never opens it on its own. Opaque: a solid fill, so the page
    /// behind never bleeds through the sidebar.
    /// Top to bottom: the current song's details, the main artist's
    /// brief [picture, details], then the next song in the queue
    /// with the Show full queue button (which opens the queue
    /// overlay over the whole sidebar). The close button and the
    /// player bar's Now playing toggle hide it; it stays hidden
    /// until the toggle is pressed again.
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
                        .corner_radius(CornerRadius::uniform(4.0))
                        .content(
                            views::image_from_url(&url)
                                .width(308.0)
                                .height(308.0)
                                .stretch(Stretch::UniformToFill),
                        ),
                    None => Border::new()
                        .width(308.0)
                        .height(308.0)
                        .corner_radius(CornerRadius::uniform(4.0))
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
            // through the sidebar.
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
                    views::song_row(song, &queue, true, self, context),
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
