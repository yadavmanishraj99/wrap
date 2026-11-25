//! Shared view helpers: formatting, artwork URLs, and the song row
//! used by Home, Search and Library.
//!
//! windows-reactor 0.100 API notes: layout props (`margin`,
//! `grid_column`, ...) are set on the builder *before* the terminal
//! `.content()` / `.children()` call; `Image::source` returns
//! `Result`; `FontWeight` uses `SEMI_BOLD` / `BOLD` constants.

use saavnapi::{DownloadLink, Song};
use windows_reactor::*;

use crate::app::{Message, Shell};

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

/// One song row: artwork, title/artist, duration, play + like buttons.
/// `queue` is the full list the row belongs to, so Play starts the
/// queue at this song's position.
pub fn song_row(
    song: &Song,
    queue: &[Song],
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> View {
    let index = queue.iter().position(|s| s.id == song.id).unwrap_or(0);
    let liked = shell.library.is_liked(&song.id);
    let duration = song
        .duration
        .map(|d| fmt_time(d as f64))
        .unwrap_or_default();

    let art_view: View = match art_url(&song.image, "150x150") {
        Some(url) => image_from_url(&url)
            .width(48.0)
            .height(48.0)
            .stretch(Stretch::UniformToFill)
            .grid_column(0)
            .into(),
        None => Border::new()
            .width(48.0)
            .height(48.0)
            .grid_column(0)
            .content(
                SymbolIcon::new()
                    .symbol(Symbol::Audio)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center),
            ),
    };

    Grid::new()
        .columns([
            GridLength::Pixel(48.0),
            GridLength::Star(1.0),
            GridLength::Auto,
            GridLength::Auto,
            GridLength::Auto,
        ])
        .margin(4.0)
        .children((
            art_view,
            StackPanel::new()
                .margin(12.0)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1)
                .children((
                    TextBlock::new()
                        .text(song.name.clone())
                        .font_size(14.0)
                        .font_weight(FontWeight::SEMI_BOLD),
                    TextBlock::new()
                        .text(artist_names(song))
                        .font_size(12.0)
                        .opacity(0.7),
                )),
            TextBlock::new()
                .text(duration)
                .font_size(12.0)
                .opacity(0.7)
                .vertical_alignment(VerticalAlignment::Center)
                .margin(12.0)
                .grid_column(2),
            Button::new()
                .margin(4.0)
                .grid_column(3)
                .on_click(context.message(Message::ToggleLike(song.clone())))
                .content(SymbolIcon::new().symbol(if liked {
                    Symbol::Favorite
                } else {
                    Symbol::Add
                })),
            Button::new()
                .margin(4.0)
                .grid_column(4)
                .on_click(context.message(Message::PlaySong(queue.to_vec(), index)))
                .content(SymbolIcon::new().symbol(Symbol::Play)),
        ))
}
