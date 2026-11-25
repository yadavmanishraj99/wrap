//! Album page: artwork, title, clickable artist names, year and
//! song count, Play and Like, then the track list.

use windows_reactor::*;

use crate::app::{DetailsContent, DetailsTarget, Message, Shell};
use crate::views;

pub fn view(
    shell: &Shell,
    context: &mut ViewContext<Shell>,
    target: &DetailsTarget,
    content: &DetailsContent,
) -> View {
    let Some(album) = &content.album else {
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
    let liked = shell.catalog.is_album_liked(&album.id);
    let meta = match (album.year, album.song_count) {
        (Some(year), Some(count)) => format!("{year} • {count} songs"),
        (Some(year), None) => format!("{year} • {} songs", content.songs.len()),
        (None, Some(count)) => format!("{count} songs"),
        (None, None) => format!("{} songs", content.songs.len()),
    };
    let album_for_like = album.clone();

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
                    TextBlock::new().text("Album").font_size(12.0).opacity(0.7),
                    TextBlock::new()
                        .text(target.title.clone())
                        .font_size(28.0)
                        .font_weight(FontWeight::BOLD)
                        .text_wrapping(TextWrapping::Wrap),
                    views::artist_links(&album.artists, 2, 120, context),
                    TextBlock::new().text(meta).font_size(14.0).opacity(0.7),
                    views::details_action_row(
                        &content.songs,
                        Some((
                            liked,
                            Message::ToggleLikeAlbum(album_for_like),
                            album.id.clone(),
                        )),
                        target,
                        shell,
                        context,
                    ),
                )),
        ));

    let mut sections: Vec<KeyedView> = vec![KeyedView::new("header", header)];
    // While this album is downloading, show its overall progress
    // above the song list (each song row shows its own progress
    // on its art at the same time).
    if let Some(progress) = views::collection_download_progress(&target.id, shell) {
        sections.push(KeyedView::new("download", progress));
    }
    if !content.songs.is_empty() {
        let rows: Vec<KeyedView> = content
            .songs
            .iter()
            .map(|song| {
                KeyedView::new(
                    song.id.clone(),
                    views::song_row(song, &content.songs, false, shell, context),
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

    StackPanel::new()
        .margin(24.0)
        .spacing(24.0)
        .keyed_children(sections)
}
