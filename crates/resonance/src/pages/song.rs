//! Song page: artwork, title, clickable artist names, Play and
//! Like, then lyrics when the API has them. Reached from the player
//! bar's lyrics button — song cards play instead of opening this.

use windows_reactor::*;

use crate::app::{DetailsContent, DetailsTarget, Message, Shell};
use crate::views;

pub fn view(
    shell: &Shell,
    context: &mut ViewContext<Shell>,
    target: &DetailsTarget,
    content: &DetailsContent,
) -> View {
    let Some(song) = &content.song else {
        return TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into();
    };
    let art_content: View = match &content.art {
        Some(url) => views::image_from_url(url)
            .width(200.0)
            .height(200.0)
            .stretch(Stretch::UniformToFill)
            .into(),
        None => SymbolIcon::new()
            .symbol(Symbol::Audio)
            .horizontal_alignment(HorizontalAlignment::Center)
            .vertical_alignment(VerticalAlignment::Center)
            .into(),
    };
    let art_view: View = Border::new()
        .width(200.0)
        .height(200.0)
        .corner_radius(CornerRadius::uniform(4.0))
        .grid_column(0)
        .content(art_content);
    let liked = shell.catalog.is_liked(&song.id);
    let song_for_like = song.clone();
    let song_for_play = song.clone();

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
                    TextBlock::new().text("Song").font_size(12.0).opacity(0.7),
                    TextBlock::new()
                        .text(target.title.clone())
                        .font_size(28.0)
                        .font_weight(FontWeight::BOLD)
                        .text_wrapping(TextWrapping::Wrap),
                    views::artist_links(&song.artists, 2, 120, context),
                    views::details_action_row(
                        std::slice::from_ref(&song_for_play),
                        Some((liked, Message::ToggleLike(song_for_like), song.id.clone())),
                        target,
                        shell,
                        context,
                    ),
                )),
        ));

    let mut sections: Vec<KeyedView> = vec![KeyedView::new("header", header)];
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

    StackPanel::new()
        .margin(24.0)
        .spacing(24.0)
        .keyed_children(sections)
}
