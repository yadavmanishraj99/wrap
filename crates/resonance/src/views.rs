//! Shared view helpers: formatting, artwork URLs, and the song row
//! used by Home, Search and Library.
//!
//! windows-reactor 0.100 API notes: layout props (`margin`,
//! `grid_column`, ...) are set on the builder *before* the terminal
//! `.content()` / `.children()` call; `Image::source` returns
//! `Result`; `FontWeight` uses `SEMI_BOLD` / `BOLD` constants.

use saavnapi::{ArtistGroups, ArtistMap, DownloadLink, Song};
use windows_reactor::*;

use crate::app::{DetailsTarget, Message, Shell};
use crate::store::Theme;

/// Seconds as `m:ss`.
pub fn fmt_time(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "0:00".to_string();
    }
    let total = secs.round() as u64;
    format!("{}:{:02}", total / 60, total % 60)
}

/// Pick an artwork URL from a list of image links (`50x50` /
/// `150x150` / `500x500`), falling back to the largest available.
pub fn art_url(links: &[DownloadLink], quality: &str) -> Option<String> {
    links
        .iter()
        .find(|link| link.quality == quality)
        .or_else(|| links.last())
        .map(|link| link.url.clone())
}

/// Build an `Image` from a URL; an invalid URL yields a blank image
/// instead of an error (`Image::source` validates the URI).
pub fn image_from_url(url: &str) -> Image {
    Image::new().source(url).unwrap_or_else(|_| Image::new())
}

/// Comma-joined primary artist names, or "Unknown artist".
pub fn artist_names(song: &Song) -> String {
    if song.artists.primary.is_empty() {
        return "Unknown artist".to_string();
    }
    song.artists
        .primary
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A section heading (WinUI subtitle style, 20px semibold).
pub fn section_header(title: &str) -> View {
    TextBlock::new()
        .text(title)
        .font_size(20.0)
        .font_weight(FontWeight::SEMI_BOLD)
        .into()
}

/// One section of media cards, Home-style: heading + up to 20 cards
/// in one row (<= 10 items) or two rows (> 10 items), scrolling
/// horizontally with the bar hidden. Shared by Home, Search and
/// Library so the three pages lay out identically.
pub fn card_section(
    title: &str,
    targets: &[DetailsTarget],
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> View {
    let shown: Vec<&DetailsTarget> = targets.iter().take(20).collect();
    if shown.is_empty() {
        return View::empty();
    }
    let mut row = |slice: &[&DetailsTarget]| -> View {
        let cards: Vec<KeyedView> = slice
            .iter()
            .map(|target| {
                KeyedView::new(
                    target.id.clone(),
                    media_card(
                        (*target).clone(),
                        target.art.clone(),
                        shell.hovered_card.as_deref() == Some(target.id.as_str()),
                        shell,
                        context,
                    ),
                )
            })
            .collect();
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(12.0)
            .keyed_children(cards)
    };
    let body: View = if shown.len() <= 10 {
        row(&shown)
    } else {
        let (first, second) = shown.split_at(10);
        StackPanel::new()
            .spacing(20.0)
            .children((row(first), row(second)))
    };
    StackPanel::new().spacing(8.0).children((
        section_header(title),
        ScrollViewer::new()
            .horizontal_scroll_bar_visibility(ScrollBarVisibility::Hidden)
            .vertical_scroll_bar_visibility(ScrollBarVisibility::Disabled)
            .content(body),
    ))
}

/// Clickable artist names: each name opens its artist page. Names
/// are laid out greedily across up to `max_rows` rows of about
/// `chars_per_row` characters (separated by ", "); overflow names
/// are dropped. Primary artists first, then featured, then all.
/// With no artists, plain "Unknown artist" text.
pub fn artist_links(
    groups: &ArtistGroups,
    max_rows: usize,
    chars_per_row: usize,
    context: &mut ViewContext<Shell>,
) -> View {
    let artists: &[ArtistMap] = if !groups.primary.is_empty() {
        &groups.primary
    } else if !groups.featured.is_empty() {
        &groups.featured
    } else {
        &groups.all
    };
    if artists.is_empty() {
        return TextBlock::new()
            .text("Unknown artist")
            .font_size(12.0)
            .opacity(0.7)
            .into();
    }
    let mut rows: Vec<Vec<&ArtistMap>> = vec![Vec::new()];
    for artist in artists {
        let cost = artist.name.chars().count() + 2; // name + ", "
        let current_len: usize = rows
            .last()
            .unwrap()
            .iter()
            .map(|a| a.name.chars().count() + 2)
            .sum();
        if !rows.last().unwrap().is_empty() && current_len + cost > chars_per_row {
            if rows.len() >= max_rows {
                break;
            }
            rows.push(Vec::new());
        }
        rows.last_mut().unwrap().push(artist);
    }
    let row_views: Vec<KeyedView> = rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let mut items: Vec<KeyedView> = Vec::new();
            for (index, artist) in row.iter().enumerate() {
                if index > 0 {
                    items.push(KeyedView::new(
                        format!("{row_index}-{index}-sep"),
                        TextBlock::new().text(", ").font_size(12.0).opacity(0.7),
                    ));
                }
                let target = DetailsTarget {
                    kind: "artist".to_string(),
                    id: artist.id.clone(),
                    title: artist.name.clone(),
                    subtitle: String::new(),
                    art: art_url(&artist.image, "500x500"),
                };
                items.push(KeyedView::new(
                    format!("{row_index}-{index}"),
                    Border::new()
                        .background(Brush::Solid(Color::transparent()))
                        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                            if info.is_left_button_pressed {
                                Message::OpenDetails(target.clone())
                            } else {
                                Message::Noop
                            }
                        }))
                        .content(
                            TextBlock::new()
                                .text(artist.name.clone())
                                .font_size(12.0)
                                .opacity(0.7),
                        ),
                ));
            }
            KeyedView::new(
                format!("row-{row_index}"),
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .keyed_children(items),
            )
        })
        .collect();
    StackPanel::new().keyed_children(row_views)
}

/// A media card: square art-fill artwork (138px), title and
/// subtitle (max 2 lines each), everything top-aligned.
///
/// Clicking the art or the text opens the details page; right-click
/// opens the context menu. While hovered, two outlined circular
/// buttons appear over the art, bottom-left (Play) and bottom-right
/// (more — opens the context menu). The buttons are siblings of the
/// artwork, not children of it, so their presses never also trigger
/// the card's own press handler.
///
/// windows-reactor 0.100 note: `Border` is the only view with
/// pointer events, so every press/hover handler sits on a Border.
pub fn media_card(
    target: DetailsTarget,
    art: Option<String>,
    hovered: bool,
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> View {
    let size = 168.0;
    let card_press = |t: DetailsTarget, context: &mut ViewContext<Shell>| {
        context.callback(move |info: PointerEventInfo| Message::CardPressed(t.clone(), info))
    };
    let art_view: View = match art {
        Some(url) => image_from_url(&url)
            .width(size)
            .height(size)
            .stretch(Stretch::UniformToFill)
            .into(),
        // No art: a music-note icon. The tile behind it is a solid
        // placeholder fill (not transparent), so the card reads as
        // a tile instead of an empty hole.
        None => SymbolIcon::new()
            .symbol(Symbol::Audio)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    };
    let mut art_children: Vec<KeyedView> = vec![KeyedView::new(
        "art",
        Border::new()
            .width(size)
            .height(size)
            // Artist art is a full circle; every other kind is a
            // 4px rounded square.
            .corner_radius(CornerRadius::uniform(if target.kind == "artist" {
                size / 2.0
            } else {
                4.0
            }))
            .background(skeleton_fill(shell.settings.theme))
            .on_pointer_pressed(card_press(target.clone(), context))
            .content(art_view),
    )];
    // Keep the hover buttons mounted while the context menu is open
    // for this card: opening the menu moves the pointer onto the
    // menu, which would otherwise fire PointerExited and yank the
    // pressed More button out of the tree mid-press.
    let menu_open = shell
        .menu
        .as_ref()
        .is_some_and(|m| m.target.kind == target.kind && m.target.id == target.id);
    if hovered || menu_open {
        let play_target = target.clone();
        art_children.push(KeyedView::new(
            "play",
            hover_button(
                Symbol::Play,
                HorizontalAlignment::Left,
                move |_info: PointerEventInfo| {
                    Message::PlayByKind(play_target.kind.clone(), play_target.id.clone())
                },
                context,
            ),
        ));
        let more_target = target.clone();
        art_children.push(KeyedView::new(
            "more",
            hover_button(
                Symbol::More,
                HorizontalAlignment::Right,
                move |info: PointerEventInfo| {
                    Message::OpenMenu(more_target.clone(), info.window_x, info.window_y)
                },
                context,
            ),
        ));
    }
    let hover_id = target.id.clone();
    Border::new()
        .width(size)
        // A navigation-like hover background: a card fill while the
        // card is hovered (or its menu is open), transparent
        // otherwise. The card already shows Play/More buttons on
        // hover; the background makes the hover state unmistakable.
        .background(if hovered || menu_open {
            Brush::Theme(ThemeBrush::CardBackground)
        } else {
            Brush::Solid(Color::transparent())
        })
        .corner_radius(CornerRadius::uniform(8.0))
        .on_pointer_entered(
            context.callback(move |_info: PointerEventInfo| {
                Message::CardHover(Some(hover_id.clone()))
            }),
        )
        .on_pointer_exited(context.callback(|_info: PointerEventInfo| Message::CardHover(None)))
        .content(
            StackPanel::new()
                .vertical_alignment(VerticalAlignment::Top)
                .children((
                    Grid::new()
                        .width(size)
                        .height(size)
                        .keyed_children(art_children),
                    Border::new()
                        .background(Brush::Solid(Color::transparent()))
                        .on_pointer_pressed(card_press(target.clone(), context))
                        .content(
                            StackPanel::new().children((
                                TextBlock::new()
                                    .text(target.title.clone())
                                    .font_size(14.0)
                                    .font_weight(FontWeight::SEMI_BOLD)
                                    .text_wrapping(TextWrapping::Wrap)
                                    .max_lines(2)
                                    .margin(4.0),
                                TextBlock::new()
                                    .text(target.subtitle.clone())
                                    .font_size(12.0)
                                    .opacity(0.7)
                                    .text_wrapping(TextWrapping::Wrap)
                                    .max_lines(2)
                                    .margin(4.0),
                            )),
                        ),
                )),
        )
}

/// One outlined circular icon button for the card hover overlay
/// (36px disc, theme card fill + card-stroke outline). A `Border`
/// is used because `Button` has no corner radius in 0.100; presses
/// are pointer presses, and the button is a sibling of the artwork.
fn hover_button(
    icon: Symbol,
    align: HorizontalAlignment,
    on_press: impl Fn(PointerEventInfo) -> Message + Clone + 'static,
    context: &mut ViewContext<Shell>,
) -> View {
    Border::new()
        .width(36.0)
        .height(36.0)
        .corner_radius(CornerRadius::uniform(18.0))
        .border_thickness(1.0)
        .border_brush(Brush::Theme(ThemeBrush::CardStroke))
        .background(Brush::Theme(ThemeBrush::CardBackground))
        .horizontal_alignment(align)
        .vertical_alignment(VerticalAlignment::Bottom)
        .margin(8.0)
        .on_pointer_pressed(context.callback(on_press))
        .content(SymbolIcon::new().symbol(icon))
}

/// A small circular like button. When not liked, it is Add (plus)
/// inside a 32px circle outline (a 2px border, transparent
/// background); the icon inside is ~16px, so there is padding
/// between the icon and the circle. When already liked, it is a
/// check inside a filled circle, the fill being the framework's
/// primary color (Accent — the same primary the play button uses;
/// #1ed760 Spotify green is the fallback if the framework ever
/// stopped providing one). The circle stroke is the same color and
/// the same weight as the icon's strokes (the theme's primary text
/// color, 2px), so the button and its circle read as one unit. The
/// not-liked button is greyish when the pointer is not over it (low
/// opacity) and goes to full contrast on hover; the liked button is
/// always full contrast, so the liked state is unmistakable. The
/// colors use the theme's foreground and stroke, dimmed by opacity,
/// so they read in both dark mode (white dimmed over the dark page)
/// and light mode (black dimmed over the light page) — no hardcoded
/// grey that would vanish on one theme.
/// `None` renders an inert greyed-out Add (no song to like yet).
pub fn like_button(
    song: Option<&Song>,
    grid_column: i32,
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> View {
    let (liked, id) = match song {
        Some(song) => (shell.library.is_liked(&song.id), Some(song.id.clone())),
        None => (false, None),
    };
    let hovered = id
        .as_deref()
        .is_some_and(|id| shell.hovered_like.as_deref() == Some(id));
    // Liked: a check inside a filled circle, the fill being the
    // framework's primary color (Accent). Not liked: a plus inside a
    // circle outline (1px border, transparent background). The liked
    // button is always full contrast; the not-liked one is greyish
    // until hovered.
    let mut border = Border::new()
        .width(32.0)
        .height(32.0)
        .corner_radius(CornerRadius::uniform(16.0))
        .background(if liked {
            Brush::Theme(ThemeBrush::Accent)
        } else {
            Brush::Solid(Color::transparent())
        })
        .opacity(if liked || hovered { 1.0 } else { 0.55 })
        .margin(4.0)
        .grid_column(grid_column);
    if !liked {
        border = border
            // A 2px stroke, the same weight as the icon's strokes,
            // and the same color as the icon (primary text).
            .border_thickness(2.0)
            .border_brush(Brush::Theme(ThemeBrush::PrimaryText));
    }
    if let Some(song) = song {
        let hover_id = song.id.clone();
        let press_song = song.clone();
        border = border
            .on_pointer_entered(context.callback(move |_info: PointerEventInfo| {
                Message::LikeHover(Some(hover_id.clone()))
            }))
            .on_pointer_exited(context.callback(|_info: PointerEventInfo| Message::LikeHover(None)))
            .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                if info.is_left_button_pressed {
                    Message::ToggleLike(press_song.clone())
                } else {
                    Message::Noop
                }
            }));
    }
    border
        .content(
            SymbolIcon::new()
                .symbol(if liked { Symbol::Accept } else { Symbol::Add })
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .tooltip(if liked {
            "Remove from liked songs"
        } else {
            "Add to liked songs"
        })
}

/// The action row under a details header, left to right: a large
/// 64x64 circular Play button (filled primary, Play icon centered),
/// a shuffle icon button (icon only, no container), a circular like
/// button (a plus in a 32px circle outline, or a check in a filled
/// primary circle when already liked), a download button (icon only,
/// in a 32px circle — a placeholder, the download feature is still
/// under discussion), and a horizontal more icon button (icon only,
/// no container) that opens the item's context menu. The icons in
/// the circular buttons are centered, with padding between the icon
/// and the circle, and the icon and the circle stroke are the same
/// color and the same weight (the theme's primary text color, 2px).
/// `like` is `Some((liked, toggle, hover_id))` for items that can be
/// liked (album, playlist, song, artist) and `None` when the item
/// has no like.
pub fn details_action_row(
    songs: &[Song],
    like: Option<(bool, Message, String)>,
    target: &DetailsTarget,
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> View {
    let play_songs = songs.to_vec();
    let shuffle_songs = songs.to_vec();
    let has_songs = !songs.is_empty();
    // The large circular Play button: 64x64, filled primary, Play
    // icon centered. Dimmed when there is nothing to play.
    let play_button = Border::new()
        .width(64.0)
        .height(64.0)
        .corner_radius(CornerRadius::uniform(32.0))
        .background(Brush::Theme(ThemeBrush::Accent))
        .opacity(if has_songs { 1.0 } else { 0.4 })
        .vertical_alignment(VerticalAlignment::Center)
        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
            if has_songs && info.is_left_button_pressed {
                Message::PlaySong(play_songs.clone(), 0)
            } else {
                Message::Noop
            }
        }))
        .content(
            SymbolIcon::new()
                .symbol(Symbol::Play)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .tooltip("Play");
    // Shuffle: icon only, no container — a transparent border with
    // the icon centered, greyish.
    let shuffle_button = Border::new()
        .width(32.0)
        .height(32.0)
        .background(Brush::Solid(Color::transparent()))
        .opacity(0.7)
        .vertical_alignment(VerticalAlignment::Center)
        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
            if info.is_left_button_pressed {
                Message::ShufflePlay(shuffle_songs.clone())
            } else {
                Message::Noop
            }
        }))
        .content(
            SymbolIcon::new()
                .symbol(Symbol::Shuffle)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .tooltip("Shuffle");
    // Download: icon only, in a circle — a 32px outline circle
    // with the icon centered, so there is padding between the icon
    // and the circle, the icon and the circle stroke the same color
    // and the same weight. A placeholder for now; the download
    // feature is still under discussion, so it does nothing yet.
    let download_button = Border::new()
        .width(32.0)
        .height(32.0)
        .corner_radius(CornerRadius::uniform(16.0))
        .background(Brush::Solid(Color::transparent()))
        .border_thickness(2.0)
        .border_brush(Brush::Theme(ThemeBrush::PrimaryText))
        .opacity(0.7)
        .vertical_alignment(VerticalAlignment::Center)
        .on_pointer_pressed(context.callback(|_info: PointerEventInfo| Message::Noop))
        .content(
            SymbolIcon::new()
                .symbol(Symbol::Download)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .tooltip("Download");
    // More: horizontal ellipsis, icon only, no container. Opens the
    // item's context menu at the click position.
    let more_target = target.clone();
    let more_button = Border::new()
        .width(32.0)
        .height(32.0)
        .background(Brush::Solid(Color::transparent()))
        .opacity(0.7)
        .vertical_alignment(VerticalAlignment::Center)
        .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
            if info.is_left_button_pressed {
                Message::OpenMenu(more_target.clone(), info.window_x, info.window_y)
            } else {
                Message::Noop
            }
        }))
        .content(
            SymbolIcon::new()
                .symbol(Symbol::More)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center),
        )
        .tooltip("More");

    let mut items: Vec<KeyedView> = vec![
        KeyedView::new("play", play_button),
        KeyedView::new("shuffle", shuffle_button),
    ];
    if let Some((liked, toggle, hover_id)) = like {
        // The circular like button: a plus in a circle outline, or a
        // check in a filled primary circle when already liked. The
        // icon is centered, and the icon and the circle stroke are
        // the same color. Greyish until hovered; the liked button is
        // always full contrast.
        let like_hovered = shell.hovered_like.as_deref() == Some(hover_id.as_str());
        let mut border = Border::new()
            .width(32.0)
            .height(32.0)
            .corner_radius(CornerRadius::uniform(16.0))
            .background(if liked {
                Brush::Theme(ThemeBrush::Accent)
            } else {
                Brush::Solid(Color::transparent())
            })
            .opacity(if liked || like_hovered { 1.0 } else { 0.55 })
            .vertical_alignment(VerticalAlignment::Center);
        if !liked {
            border = border
                // A 2px stroke, the same weight as the icon's
                // strokes, and the same color as the icon.
                .border_thickness(2.0)
                .border_brush(Brush::Theme(ThemeBrush::PrimaryText));
        }
        let hover_id_enter = hover_id.clone();
        let like_button = border
            .on_pointer_entered(context.callback(move |_info: PointerEventInfo| {
                Message::LikeHover(Some(hover_id_enter.clone()))
            }))
            .on_pointer_exited(context.callback(|_info: PointerEventInfo| Message::LikeHover(None)))
            .on_pointer_pressed(context.callback(move |info: PointerEventInfo| {
                if info.is_left_button_pressed {
                    toggle.clone()
                } else {
                    Message::Noop
                }
            }))
            .content(
                SymbolIcon::new()
                    .symbol(if liked { Symbol::Accept } else { Symbol::Add })
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center),
            )
            .tooltip(if liked {
                "Remove from liked"
            } else {
                "Add to liked"
            });
        items.push(KeyedView::new("like", like_button));
    }
    items.push(KeyedView::new("download", download_button));
    items.push(KeyedView::new("more", more_button));

    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(16.0)
        .keyed_children(items)
}

/// One song row: artwork, title/artist, and — unless `compact` —
/// a favorite button and the duration. `queue` is the full list the
/// row belongs to, so Play starts the queue at this song's position.
/// The title and the artist line each wrap to at most 2 lines, the
/// same rule as the media cards. The row gets a navigation-like
/// hover background (a card fill while hovered). On hover, a circular
/// play icon appears over the art (the same hover affordance as the
/// media cards); clicking it plays the song. There is no permanent
/// Play button — the hover icon is the play affordance.
///
/// In the full (details) form, the favorite button sits before the
/// duration, with a gap between them, and only shows while the row
/// is hovered. In the compact form (the Now Playing queue), there is
/// no favorite button and no duration — just art, title/artist, and
/// the hover background.
pub fn song_row(
    song: &Song,
    queue: &[Song],
    compact: bool,
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> View {
    let index = queue.iter().position(|s| s.id == song.id).unwrap_or(0);
    let duration = song
        .duration
        .map(|d| fmt_time(d as f64))
        .unwrap_or_default();
    let hovered = shell.hovered_card.as_deref() == Some(song.id.as_str());

    let art_border: View = match art_url(&song.image, "150x150") {
        Some(url) => Border::new()
            .width(48.0)
            .height(48.0)
            .corner_radius(CornerRadius::uniform(4.0))
            .content(
                image_from_url(&url)
                    .width(48.0)
                    .height(48.0)
                    .stretch(Stretch::UniformToFill),
            ),
        None => Border::new()
            .width(48.0)
            .height(48.0)
            .corner_radius(CornerRadius::uniform(4.0))
            .content(
                SymbolIcon::new()
                    .symbol(Symbol::Audio)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center),
            ),
    };
    let mut art_children: Vec<KeyedView> = vec![KeyedView::new("art", art_border)];
    if hovered {
        let play_queue = queue.to_vec();
        art_children.push(KeyedView::new(
            "play",
            Border::new()
                .width(32.0)
                .height(32.0)
                .corner_radius(CornerRadius::uniform(16.0))
                .border_thickness(1.0)
                .border_brush(Brush::Theme(ThemeBrush::CardStroke))
                .background(Brush::Theme(ThemeBrush::CardBackground))
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center)
                .on_pointer_pressed(context.callback(move |_info: PointerEventInfo| {
                    Message::PlaySong(play_queue.clone(), index)
                }))
                .content(SymbolIcon::new().symbol(Symbol::Play)),
        ));
    }
    let art_view: View = Grid::new()
        .width(48.0)
        .height(48.0)
        .grid_column(0)
        .keyed_children(art_children);

    let title_panel = StackPanel::new()
        .margin(12.0)
        .vertical_alignment(VerticalAlignment::Center)
        .grid_column(1)
        .children((
            TextBlock::new()
                .text(song.name.clone())
                .font_size(14.0)
                .font_weight(FontWeight::SEMI_BOLD)
                .text_wrapping(TextWrapping::Wrap)
                .max_lines(2),
            TextBlock::new()
                .text(artist_names(song))
                .font_size(12.0)
                .opacity(0.7)
                .text_wrapping(TextWrapping::Wrap)
                .max_lines(2),
        ));
    // The full form: favorite before the duration, with a gap
    // between them, and the favorite only shows while hovered. The
    // compact form (Now Playing queue) has neither.
    let (columns, row_children): (Vec<GridLength>, Vec<KeyedView>) = if compact {
        (
            vec![GridLength::Pixel(48.0), GridLength::Star(1.0)],
            vec![
                KeyedView::new("art", art_view),
                KeyedView::new("title", title_panel),
            ],
        )
    } else {
        let mut children = vec![
            KeyedView::new("art", art_view),
            KeyedView::new("title", title_panel),
        ];
        if hovered {
            children.push(KeyedView::new(
                "favorite",
                like_button(Some(song), 2, shell, context),
            ));
        }
        children.push(KeyedView::new(
            "duration",
            TextBlock::new()
                .text(duration)
                .font_size(12.0)
                .opacity(0.7)
                .vertical_alignment(VerticalAlignment::Center)
                .margin(12.0)
                .grid_column(3),
        ));
        (
            vec![
                GridLength::Pixel(48.0),
                GridLength::Star(1.0),
                GridLength::Auto,
                GridLength::Auto,
            ],
            children,
        )
    };

    let hover_id = song.id.clone();
    Border::new()
        .background(if hovered {
            Brush::Theme(ThemeBrush::CardBackground)
        } else {
            Brush::Solid(Color::transparent())
        })
        .corner_radius(CornerRadius::uniform(4.0))
        .on_pointer_entered(
            context.callback(move |_info: PointerEventInfo| {
                Message::CardHover(Some(hover_id.clone()))
            }),
        )
        .on_pointer_exited(context.callback(|_info: PointerEventInfo| Message::CardHover(None)))
        .content(
            Grid::new()
                .columns(columns)
                .margin(4.0)
                .keyed_children(row_children),
        )
}

// ---------------------------------------------------------------------------
// Skeleton loading shapes.
//
// A skeleton is a set of rounded placeholder bars in the theme's card
// color, the same size and layout as the content that will replace
// it. They are static — no shimmer pulse, no fade: motion is not
// supported, so the placeholders just sit at full opacity until the
// real content replaces them.
// ---------------------------------------------------------------------------

/// The skeleton fill: a solid gray with real contrast against the
/// page background in both themes. Checked with the WCAG contrast
/// formula (page: dark #202020, light #F3F3F3): dark #4A4A4A is
/// 1.84:1, light #BCBCBC is 1.71:1. The old CardBackground fill was
/// ~1.1:1 in both themes — nearly invisible. `Theme::System` follows
/// the app's dark-first default.
pub fn skeleton_fill(theme: Theme) -> Brush {
    match theme {
        Theme::Light => Brush::Solid(Color::rgb(0xBC, 0xBC, 0xBC)),
        _ => Brush::Solid(Color::rgb(0x4A, 0x4A, 0x4A)),
    }
}

/// A rounded placeholder bar (a line of text, a heading, a button).
pub fn skeleton_bar(width: f64, height: f64, shell: &Shell) -> View {
    Border::new()
        .width(width)
        .height(height)
        .corner_radius(CornerRadius::uniform(height / 2.0))
        .background(skeleton_fill(shell.settings.theme))
        .into()
}

/// A square placeholder for artwork (a card image, a details-page
/// cover). `size` is the side length; corners are rounded 4px, the
/// same as the real artwork it stands in for.
pub fn skeleton_art(size: f64, shell: &Shell) -> View {
    Border::new()
        .width(size)
        .height(size)
        .corner_radius(CornerRadius::uniform(4.0))
        .background(skeleton_fill(shell.settings.theme))
        .into()
}

/// A media-card-shaped skeleton: 168px art + title + subtitle bars,
/// same size and spacing as a real card.
pub fn skeleton_card(shell: &Shell) -> View {
    StackPanel::new().spacing(8.0).children((
        skeleton_art(168.0, shell),
        skeleton_bar(120.0, 12.0, shell),
        skeleton_bar(84.0, 10.0, shell),
    ))
}

/// A song-row-shaped skeleton: 48px art + title/artist bars.
pub fn skeleton_row(shell: &Shell) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(12.0)
        .margin(4.0)
        .children((
            Border::new()
                .width(48.0)
                .height(48.0)
                .corner_radius(CornerRadius::uniform(4.0))
                .background(skeleton_fill(shell.settings.theme)),
            StackPanel::new()
                .vertical_alignment(VerticalAlignment::Center)
                .spacing(8.0)
                .children((
                    skeleton_bar(180.0, 12.0, shell),
                    skeleton_bar(120.0, 10.0, shell),
                )),
        ))
}

/// A section-shaped skeleton: the section's REAL heading (the
/// layout's static text) + one row of `cards` skeleton cards, 12px
/// apart — the same shape as the real section, so the placeholder
/// reads as the page, not a generic stack of bars.
pub fn skeleton_section(shell: &Shell, title: &str, cards: usize) -> View {
    let mut card_views: Vec<KeyedView> = Vec::new();
    for i in 0..cards {
        card_views.push(KeyedView::new(format!("card{i}"), skeleton_card(shell)));
    }
    StackPanel::new().spacing(8.0).children((
        section_header(title),
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(12.0)
            .keyed_children(card_views),
    ))
}

/// A button-shaped placeholder (Play / Like), the same rounded
/// rectangle a real Button is — corner radius 4, not a pill.
pub fn skeleton_button(width: f64, height: f64, shell: &Shell) -> View {
    Border::new()
        .width(width)
        .height(height)
        .corner_radius(CornerRadius::uniform(4.0))
        .background(skeleton_fill(shell.settings.theme))
        .into()
}
