//! Details page: what a media card opens. Shows the artwork, title
//! and subtitle, a Play button (plays the full list, or the single
//! song), the song list, and lyrics for songs when the API has them.
//! The title bar's back button returns to the page we came from.

use windows_reactor::*;

use crate::app::{Message, Shell};
use crate::views;

/// Details page root.
pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let Some(details) = &shell.details else {
        return TextBlock::new()
            .text("Nothing selected.")
            .margin(24.0)
            .into();
    };
    if details.loading {
        return StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text(details.target.title.clone())
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            ProgressRing::new().is_active(true),
            TextBlock::new()
                .text("Loading details...")
                .font_size(14.0)
                .opacity(0.7),
        ));
    }
    if let Some(err) = &details.error {
        return StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text(details.target.title.clone())
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text(format!("Couldn't load details: {err}"))
                .font_size(14.0)
                .opacity(0.7),
            Button::new()
                .on_click(context.message(Message::Back))
                .content("Back"),
        ));
    }
    let Some(content) = &details.content else {
        return TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into();
    };

    // Header: artwork + title/subtitle + actions.
    let art_view: View = match &content.art {
        Some(url) => views::image_from_url(url)
            .width(200.0)
            .height(200.0)
            .stretch(Stretch::UniformToFill)
            .grid_column(0)
            .into(),
        None => Border::new()
            .width(200.0)
            .height(200.0)
            .grid_column(0)
            .content(
                SymbolIcon::new()
                    .symbol(Symbol::Audio)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center),
            ),
    };
    let mut actions: Vec<KeyedView> = vec![KeyedView::new(
        "play",
        Button::new()
            .is_enabled(!content.songs.is_empty())
            .on_click(context.message(Message::PlaySong(content.songs.clone(), 0)))
            .content(
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(8.0)
                    .children((
                        SymbolIcon::new().symbol(Symbol::Play),
                        TextBlock::new().text("Play"),
                    )),
            ),
    )];
    if let Some(song) = &content.song {
        let liked = shell.library.liked_songs.iter().any(|s| s.id == song.id);
        actions.push(KeyedView::new(
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
                            TextBlock::new().text(if liked { "Liked" } else { "Like" }),
                        )),
                ),
        ));
    }

    let header = Grid::new()
        .columns([GridLength::Pixel(200.0), GridLength::Star(1.0)])
        .children((
            art_view,
            StackPanel::new()
                .margin(24.0)
                .spacing(8.0)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1)
                .children((
                    TextBlock::new()
                        .text(details.target.title.clone())
                        .font_size(28.0)
                        .font_weight(FontWeight::BOLD)
                        .text_wrapping(TextWrapping::Wrap),
                    TextBlock::new()
                        .text(content.subtitle.clone())
                        .font_size(14.0)
                        .opacity(0.7)
                        .text_wrapping(TextWrapping::Wrap),
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(8.0)
                        .keyed_children(actions),
                )),
        ));

    let mut sections: Vec<KeyedView> = vec![KeyedView::new("header", header)];
    if !content.songs.is_empty() {
        let rows: Vec<KeyedView> = content
            .songs
            .iter()
            .map(|song| {
                KeyedView::new(
                    song.id.clone(),
                    views::song_row(song, &content.songs, shell, context),
                )
            })
            .collect();
        sections.push(KeyedView::new(
            "songs",
            StackPanel::new().spacing(4.0).children((
                views::section_header("Songs"),
                StackPanel::new().spacing(4.0).keyed_children(rows),
            )),
        ));
    }
    if let Some(lyrics) = &content.lyrics {
        sections.push(KeyedView::new(
            "lyrics",
            StackPanel::new().spacing(8.0).children((
                views::section_header("Lyrics"),
                TextBlock::new()
                    .text(lyrics.clone())
                    .font_size(14.0)
                    .text_wrapping(TextWrapping::Wrap),
            )),
        ));
    }

    ScrollViewer::new().content(
        StackPanel::new()
            .margin(24.0)
            .spacing(24.0)
            .keyed_children(sections),
    )
}
