//! The app shell: TitleBar (header search), NavigationView sidebar,
//! page content, and the bottom player bar.
//!
//! WinUI design notes (Windows app design guidelines):
//! - NavigationView in Left mode is the shell's primary navigation.
//! - The title bar hosts the search box — the app's primary command.
//! - The bottom player bar is a persistent 76px media bar: artwork,
//!   track identity, transport controls, seek, volume.
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

use saavnapi::{LaunchData, Song};
use windows_reactor::*;

use crate::data;
use crate::pages;
use crate::player::{Player, Quality};
use crate::store::{Library, Settings, Theme};
use crate::views;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Search,
    Library,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Search => "Search",
            Self::Library => "Your Library",
            Self::Settings => "Settings",
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

pub struct Shell {
    pub page: Page,
    pub pane_open: bool,
    pub search_text: String,
    pub home: Option<LaunchData>,
    pub home_loading: bool,
    pub home_error: Option<String>,
    pub search_results: Vec<Song>,
    pub search_loading: bool,
    pub search_error: Option<String>,
    pub library: Library,
    pub settings: Settings,
    pub player: Player,
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
}

#[derive(Clone)]
pub enum Message {
    Navigate(Page),
    TogglePane,
    PaneOpenChanged(bool),
    ToggleTheme,
    SearchTextChanged(String),
    SearchSubmitted,
    HomeLoaded(Result<LaunchData, String>),
    SearchLoaded(Result<Vec<Song>, String>),
    PlaySong(Vec<Song>, usize),
    PlayPause,
    Next,
    Previous,
    Seek(f64),
    VolumeChanged(f64),
    Tick,
    ToggleLike(Song),
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
        context.spawn_background(move |_| {
            let result = data::runtime_handle()
                .block_on(async { data::client().get_launch_data(None).await });
            Message::HomeLoaded(result.map_err(|err| err.to_string()))
        });

        Self {
            page: Page::Home,
            pane_open: true,
            search_text: String::new(),
            home: None,
            home_loading: true,
            home_error: None,
            search_results: Vec::new(),
            search_loading: false,
            search_error: None,
            library,
            settings,
            player,
            position: 0.0,
            duration: 0.0,
            playing: false,
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
            Message::SearchTextChanged(text) => self.search_text = text,
            Message::SearchSubmitted => {
                let query = self.search_text.trim().to_string();
                if query.is_empty() {
                    return;
                }
                self.page = Page::Search;
                self.search_loading = true;
                self.search_error = None;
                context.spawn_background(move |_| {
                    let result = data::runtime_handle()
                        .block_on(async { data::client().search_songs(&query, 0, 20).await })
                        .map(|page| page.results);
                    Message::SearchLoaded(result.map_err(|err| err.to_string()))
                });
            }
            Message::HomeLoaded(result) => {
                self.home_loading = false;
                match result {
                    Ok(data) => self.home = Some(data),
                    Err(err) => self.home_error = Some(err),
                }
            }
            Message::SearchLoaded(result) => {
                self.search_loading = false;
                match result {
                    Ok(songs) => self.search_results = songs,
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
                self.player.next(quality);
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
                self.settings.volume = volume;
                self.settings.save();
                self.player.set_volume(volume);
            }
            Message::Tick => {
                self.position = self.player.position_secs();
                self.duration = self.player.duration_secs();
                self.playing = self.player.is_playing();
                // MediaPlayer doesn't auto-advance our queue.
                if self.playing && self.duration > 5.0 && self.position >= self.duration - 0.3 {
                    let quality = Quality::from_setting(self.settings.quality);
                    self.player.next(quality);
                    self.after_track_change(context);
                } else if self.player.has_current() {
                    spawn_tick(context);
                }
            }
            Message::ToggleLike(song) => {
                self.library.toggle_like(song);
            }
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        let theme = match self.settings.theme {
            Theme::Dark => WindowTheme::Dark,
            Theme::Light => WindowTheme::Light,
            Theme::System => WindowTheme::System,
        };
        context.window_visuals(WindowVisuals::new().client_size(1280.0, 800.0).theme(theme));

        let theme_label = match self.settings.theme {
            Theme::Dark => "Theme: Dark",
            Theme::Light => "Theme: Light",
            Theme::System => "Theme: System",
        };

        let title_bar = TitleBar::new()
            .preferred_height(WindowTitleBarHeight::Tall)
            .title("Resonance")
            .subtitle(self.page.title())
            .is_back_button_visible(false)
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
                GridLength::Pixel(76.0),
            ])
            .children((title_bar, navigation, player_bar))
            .into()
    }
}

impl Shell {
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

    /// The persistent bottom player bar (76px): artwork, track
    /// identity, transport controls, seek, volume.
    fn player_bar(&self, context: &mut ViewContext<Self>) -> View {
        let (art, title, artist) = match self.player.current() {
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
                )
                .into(),
        };

        Grid::new()
            .columns([
                GridLength::Pixel(64.0),
                GridLength::Star(1.0),
                GridLength::Auto,
                GridLength::Star(2.0),
                GridLength::Auto,
            ])
            .margin(8.0)
            .grid_row(2)
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
                            .font_weight(FontWeight::SEMI_BOLD),
                        TextBlock::new().text(artist).font_size(12.0).opacity(0.7),
                    )),
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(4.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(2)
                    .children((
                        Button::new()
                            .on_click(context.message(Message::Previous))
                            .is_enabled(self.player.has_current())
                            .content(SymbolIcon::new().symbol(Symbol::Previous)),
                        Button::new()
                            .on_click(context.message(Message::PlayPause))
                            .is_enabled(self.player.has_current())
                            .content(SymbolIcon::new().symbol(if self.playing {
                                Symbol::Pause
                            } else {
                                Symbol::Play
                            })),
                        Button::new()
                            .on_click(context.message(Message::Next))
                            .is_enabled(self.player.has_current())
                            .content(SymbolIcon::new().symbol(Symbol::Next)),
                    )),
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(8.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .margin(16.0)
                    .grid_column(3)
                    .children((
                        TextBlock::new()
                            .text(views::fmt_time(self.position))
                            .font_size(12.0)
                            .opacity(0.7),
                        Slider::new()
                            .minimum(0.0)
                            .maximum(self.duration.max(1.0))
                            .value(self.position.min(self.duration.max(1.0)))
                            .on_value_changed(context.callback(|value: f64| Message::Seek(value))),
                        TextBlock::new()
                            .text(views::fmt_time(self.duration))
                            .font_size(12.0)
                            .opacity(0.7),
                    )),
                Slider::new()
                    .minimum(0.0)
                    .maximum(1.0)
                    .value(self.settings.volume)
                    .width(120.0)
                    .vertical_alignment(VerticalAlignment::Center)
                    .grid_column(4)
                    .on_value_changed(context.callback(|value: f64| Message::VolumeChanged(value))),
            ))
            .into()
    }
}
