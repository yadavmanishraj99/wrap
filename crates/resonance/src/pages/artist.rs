//! Artist page: circular artwork, name, follower count, Play (top
//! songs), then Top songs, Top albums and Similar artists sections.

use windows_reactor::*;

use crate::app::{DetailsContent, DetailsTarget, Message, Shell};
use crate::views;

pub fn view(
    shell: &Shell,
    context: &mut ViewContext<Shell>,
    target: &DetailsTarget,
    content: &DetailsContent,
) -> View {
    let Some(artist) = &content.artist else {
        return TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into();
    };
    let liked = shell.catalog.is_artist_liked(&artist.id);
    let artist_for_like = artist.clone();
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
        .corner_radius(CornerRadius::uniform(100.0))
        .grid_column(0)
        .content(art_content);

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
                    TextBlock::new().text("Artist").font_size(12.0).opacity(0.7),
                    TextBlock::new()
                        .text(target.title.clone())
                        .font_size(28.0)
                        .font_weight(FontWeight::BOLD)
                        .text_wrapping(TextWrapping::Wrap),
                    TextBlock::new()
                        .text(content.subtitle.clone())
                        .font_size(14.0)
                        .opacity(0.7)
                        .text_wrapping(TextWrapping::Wrap),
                    views::details_action_row(
                        &content.songs,
                        Some((
                            liked,
                            Message::ToggleLikeArtist(artist_for_like),
                            artist.id.clone(),
                        )),
                        target,
                        shell,
                        context,
                    ),
                )),
        ));

    let mut sections: Vec<KeyedView> = vec![KeyedView::new("header", header)];
    // While this artist's top songs are downloading, show the
    // overall progress above the song list.
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
                views::section_header("Top songs"),
                StackPanel::new().spacing(4.0).keyed_children(rows),
            )),
        ));
    }
    if let Some(albums) = &artist.top_albums
        && !albums.is_empty()
    {
        let targets: Vec<DetailsTarget> = albums
            .iter()
            .map(|album| DetailsTarget {
                kind: "album".to_string(),
                id: album.id.clone(),
                title: album.name.clone(),
                subtitle: album
                    .artists
                    .primary
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                art: views::art_url(&album.image, "500x500"),
            })
            .collect();
        sections.push(KeyedView::new(
            "albums",
            views::card_section("Top albums", &targets, shell, context),
        ));
    }
    if let Some(similar) = &artist.similar_artists
        && !similar.is_empty()
    {
        let targets: Vec<DetailsTarget> = similar
            .iter()
            .map(|other| DetailsTarget {
                kind: "artist".to_string(),
                id: other.id.clone(),
                title: other.name.clone(),
                subtitle: String::new(),
                art: views::art_url(&other.image, "500x500"),
            })
            .collect();
        sections.push(KeyedView::new(
            "similar",
            views::card_section("Similar artists", &targets, shell, context),
        ));
    }

    StackPanel::new()
        .margin(24.0)
        .spacing(24.0)
        .keyed_children(sections)
}
