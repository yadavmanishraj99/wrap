//! The app shell: TitleBar (header search), NavigationView sidebar,
//! page content, and the bottom player bar.
//!
//! WinUI design notes (Windows app design guidelines):
//! - NavigationView in Left mode is the shell's primary navigation.
//! - The title bar hosts the search box — the app's primary command.
//! - The bottom player bar is a persistent 90px media bar modeled on
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

use saavnapi::{ArtistArgs, LaunchData, SearchAll, Song};
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
    Details,
    Queue,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Search => "Search",
            Self::Library => "Your Library",
            Self::Settings => "Settings",
            Self::Details => "Details",
            Self::Queue => "Queue",
        }
    }

    fn from_tag(tag: Option<String>) -> Self {
        match tag.as_deref() {
            Some("search") => Self::Search,
            Some("library") => Self::Library,
            Some("settings") => Self::Settings,
            _ => Self::Home,
        }
    }
}

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
#[derive(Debug, Clone)]
pub struct DetailsContent {
    pub song: Option<Song>,
    pub songs: Vec<Song>,
    pub lyrics: Option<String>,
    pub subtitle: String,
    pub art: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DetailsState {
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
    SearchSubmitted,
    SuggestionChosen(String),
    SuggestionsLoaded(String, Vec<String>),
    HomeLoaded(Result<LaunchData, String>),
    SearchAllLoaded(Result<SearchAll, String>),
    PlaySong(Vec<Song>, usize),
    PlayPause,
    Next,
    Previous,
    Seek(f64),
    VolumeChanged(f64),
    Tick,
    ToggleLike(Song),
    PlayByKind(String, String),
    OpenDetails(DetailsTarget),
    DetailsLoaded(Result<DetailsContent, String>),
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

/// Fetch a song/album/playlist/artist's songs in the background and
/// start the queue at the first song. Failures are silent (Noop).
fn spawn_play_by_kind(context: &ComponentContext<Shell>, kind: String, id: String) {
    context.spawn_background(move |_| {
        let result = data::runtime_handle().block_on(async {
            let client = data::client();
            match kind.as_str() {
                "song" => client.get_songs_by_ids(&[id.as_str()]).await,
                "album" => client
                    .get_album_by_id(&id)
                    .await
                    .map(|album| album.songs.unwrap_or_default()),
                "artist" => {
                    client
                        .get_artist_top_songs(&id, "", 0, None, None, None)
                        .await
                }
                _ => client
                    .get_playlist_by_id(&id, 50, 0)
                    .await
                    .map(|playlist| playlist.songs.unwrap_or_default()),
            }
        });
        match result {
            Ok(queue) if !queue.is_empty() => Message::PlaySong(queue, 0),
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
            Ok(DetailsContent {
                song: None,
                songs: album.songs.unwrap_or_default(),
                lyrics: None,
                subtitle,
                art: views::art_url(&album.image, "500x500"),
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
            Ok(DetailsContent {
                song: None,
                songs: artist.top_songs.unwrap_or_default(),
                lyrics: None,
                subtitle,
                art: views::art_url(&artist.image, "500x500"),
            })
        }
        _ => {
            let playlist = client.get_playlist_by_id(&target.id, 100, 0).await?;
            Ok(DetailsContent {
                song: None,
                songs: playlist.songs.unwrap_or_default(),
                lyrics: None,
                subtitle: playlist.description.clone().unwrap_or_default(),
                art: views::art_url(&playlist.image, "500x500"),
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
            Message::Navigate(page) => self.page = page,
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
                // Live suggestions from the top-query section. The
                // query rides along so a slow reply can't overwrite
                // suggestions for newer text.
                let query = text.trim().to_string();
                if query.len() >= 2 {
                    context.spawn_background(move |_| {
                        let titles = data::runtime_handle()
                            .block_on(async { data::client().search_all(&query).await })
                            .map(|all| {
                                all.top_query
                                    .results
                                    .iter()
                                    .map(|hit| hit.title.clone())
                                    .take(8)
                                    .collect()
                            })
                            .unwrap_or_default();
                        Message::SuggestionsLoaded(query, titles)
                    });
                } else {
                    self.search_suggestions.clear();
                }
            }
            Message::SuggestionsLoaded(query, titles) => {
                if query == self.search_text.trim() {
                    self.search_suggestions = titles;
                }
            }
            Message::SuggestionChosen(text) => {
                self.search_text = text;
                self.submit_search(context);
            }
            Message::SearchSubmitted => {
                self.submit_search(context);
            }
            Message::HomeLoaded(result) => {
                self.home_loading = false;
                match result {
                    Ok(data) => self.home = Some(data),
                    Err(err) => self.home_error = Some(err),
                }
            }
            Message::SearchAllLoaded(result) => {
                self.search_loading = false;
                match result {
                    Ok(all) => self.search_all = Some(all),
                    Err(err) => self.search_error = Some(err),
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
            Message::PlayByKind(kind, id) => {
                spawn_play_by_kind(context, kind, id);
            }
            Message::OpenDetails(target) => {
                if self.page != Page::Details {
                    self.back_page = self.page;
                }
                self.page = Page::Details;
                self.details = Some(DetailsState {
                    target: target.clone(),
                    loading: true,
                    error: None,
                    content: None,
                });
                spawn_details_load(context, target);
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
            Message::Back => {
                self.page = self.back_page;
                self.details = None;
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

        let on_drill_in = self.page == Page::Details || self.page == Page::Queue;
        let title_bar = TitleBar::new()
            .preferred_height(WindowTitleBarHeight::Tall)
            .title("Resonance")
            .subtitle(self.page.title())
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
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(8.0)
                        .children((
                            Button::new()
                                .on_click(context.message(Message::SearchSubmitted))
                                .content(SymbolIcon::new().symbol(Symbol::Find)),
                            Button::new()
                                .on_click(context.message(Message::ToggleTheme))
                                .content(theme_label),
                        )),
                ),
            ]);

        let page_view: View = match self.page {
            Page::Home => pages::home::view(self, context),
            Page::Search => pages::search::view(self, context),
            Page::Library => pages::library::view(self, context),
            Page::Settings => pages::settings::view(self, context),
            Page::Details => pages::details::view(self, context),
            Page::Queue => pages::queue::view(self, context),
        };

        let navigation = NavigationView::new()
            .pane_display_mode(NavigationViewPaneDisplayMode::Left)
            .is_pane_toggle_button_visible(false)
            .is_back_button_visible(NavigationViewBackButtonVisible::Collapsed)
            .is_settings_visible(false)
            .always_show_header(false)
            .pane_title("Resonance")
            .is_pane_open(self.pane_open)
            .on_is_pane_open_changed(context.callback(Message::PaneOpenChanged))
            .on_selected_tag_changed(
                context.callback(|tag: Option<String>| Message::Navigate(Page::from_tag(tag))),
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
                        nav_item(
                            "settings",
                            "Settings",
                            Symbol::Setting,
                            self.page == Page::Settings,
                        ),
                    ],
                ),
                SlotView::new(NavigationViewSlot::Content, page_view),
            ]);

        let player_bar = self.player_bar(context);

        Grid::new()
            .rows([
                GridLength::Auto,
                GridLength::Star(1.0),
                GridLength::Pixel(90.0),
            ])
            .children((title_bar, navigation, player_bar))
    }
}

impl Shell {
    /// Run the current header/page query through `search_all` and
    /// show the Search page. Shared by the header button and by
    /// choosing a suggestion.
    fn submit_search(&mut self, context: &ComponentContext<Self>) {
        let query = self.search_text.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.page = Page::Search;
        self.search_loading = true;
        self.search_error = None;
        context.spawn_background(move |_| {
            let result =
                data::runtime_handle().block_on(async { data::client().search_all(&query).await });
            Message::SearchAllLoaded(result.map_err(|err| err.to_string()))
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

    /// The persistent bottom player bar (90px), modeled on Spotify's
    /// Now Playing bar. Three zones: track info (left), controls +
    /// progress stacked (center), extras (right).
    ///
    /// The play/pause button is a filled circular Border (Button has
    /// no corner radius in windows-reactor 0.100); Border is also the
    /// only control with pointer events, so it answers left presses.
    fn player_bar(&self, context: &mut ViewContext<Self>) -> View {
        let has_current = self.player.has_current();
        let current = self.player.current().cloned();

        // --- Left: 56px art, title/artist, add-to-liked button. ---
        let (art, title, artist) = match &current {
            Some(song) => (
                views::art_url(&song.image, "150x150"),
                song.name.clone(),
                views::artist_names(song),
            ),
            None => (
                None,
                "Nothing playing".to_string(),
                "Search or open Home to play a song".to_string(),
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
                            .text_trimming(TextTrimming::CharacterEllipsis),
                        TextBlock::new()
                            .text(artist)
                            .font_size(12.0)
                            .opacity(0.7)
                            .text_trimming(TextTrimming::CharacterEllipsis),
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
            .margin(16.0)
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
            .grid_row(2)
            .children((left, center, right))
    }
}
