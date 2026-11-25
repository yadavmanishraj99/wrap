//! Search page: live suggestions in an AutoSuggestBox plus the
//! full `search_all` result — top result, songs, albums, artists,
//! playlists. Song/album/playlist rows play through `PlayByKind`
//! (the lightweight hits carry no stream URLs, so details are
//! fetched in the background first).

use saavnapi::{DownloadLink, SearchAlbumHit, SearchArtistHit, SearchHit, SearchPlaylistHit};
use windows_reactor::*;

use crate::app::{Message, Shell};
use crate::views;

/// A lightweight-hit row: artwork, title/subtitle, Play button.
fn hit_row(
    id: &str,
    kind: &str,
    title: &str,
    subtitle: &str,
    image: &[DownloadLink],
    context: &mut ViewContext<Shell>,
) -> View {
    let art_view: View = match views::art_url(image, "150x150") {
        Some(url) => views::image_from_url(&url)
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
            )
            .into(),
    };

    Grid::new()
        .columns([
            GridLength::Pixel(48.0),
            GridLength::Star(1.0),
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
                        .text(title)
                        .font_size(14.0)
                        .font_weight(FontWeight::SEMI_BOLD),
                    TextBlock::new().text(subtitle).font_size(12.0).opacity(0.7),
                )),
            Button::new()
                .margin(4.0)
                .grid_column(2)
                .on_click(context.message(Message::PlayByKind(kind.to_string(), id.to_string())))
                .content(SymbolIcon::new().symbol(Symbol::Play)),
        ))
        .into()
}

fn song_subtitle(hit: &SearchHit) -> String {
    hit.primary_artists
        .clone()
        .or(hit.singers.clone())
        .or(hit.description.clone())
        .unwrap_or_else(|| hit.kind.clone().unwrap_or_default())
}

fn section(title: &str, rows: Vec<KeyedView>) -> View {
    let mut children: Vec<KeyedView> = vec![KeyedView::new("header", views::section_header(title))];
    children.extend(rows);
    StackPanel::new()
        .spacing(4.0)
        .keyed_children(children)
        .into()
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let search_box = AutoSuggestBox::new()
        .text(shell.search_text.clone())
        .placeholder_text("Search songs, albums, artists, playlists...")
        .items_source(shell.search_suggestions.clone())
        .on_text_changed(context.callback(|value: String| Message::SearchTextChanged(value)))
        .on_suggestion_chosen(context.callback(|value: String| Message::SuggestionChosen(value)));

    let content: View = if shell.search_loading {
        StackPanel::new()
            .spacing(12.0)
            .children((
                ProgressRing::new().is_active(true),
                TextBlock::new()
                    .text("Searching...")
                    .font_size(14.0)
                    .opacity(0.7),
            ))
            .into()
    } else if let Some(err) = &shell.search_error {
        TextBlock::new()
            .text(format!("Search failed: {err}"))
            .font_size(14.0)
            .opacity(0.7)
            .into()
    } else if let Some(all) = &shell.search_all {
        let mut sections: Vec<KeyedView> = Vec::new();

        if let Some(top) = all.top_query.results.first() {
            sections.push(KeyedView::new(
                "top",
                StackPanel::new().spacing(4.0).keyed_children(vec![
                    KeyedView::new("header", views::section_header("Top result")),
                    KeyedView::new(
                        format!("top-{}", top.id),
                        hit_row(
                            &top.id,
                            top.kind.as_deref().unwrap_or("song"),
                            &top.title,
                            &song_subtitle(top),
                            &top.image,
                            context,
                        ),
                    ),
                ]),
            ));
        }

        if !all.songs.results.is_empty() {
            let rows: Vec<KeyedView> = all
                .songs
                .results
                .iter()
                .take(8)
                .map(|hit| {
                    KeyedView::new(
                        hit.id.clone(),
                        hit_row(
                            &hit.id,
                            "song",
                            &hit.title,
                            &song_subtitle(hit),
                            &hit.image,
                            context,
                        ),
                    )
                })
                .collect();
            sections.push(KeyedView::new("songs", section("Songs", rows)));
        }

        if !all.albums.results.is_empty() {
            let rows: Vec<KeyedView> = all
                .albums
                .results
                .iter()
                .take(5)
                .map(|hit: &SearchAlbumHit| {
                    KeyedView::new(
                        hit.id.clone(),
                        hit_row(
                            &hit.id,
                            "album",
                            &hit.title,
                            hit.artist.as_deref().unwrap_or(""),
                            &hit.image,
                            context,
                        ),
                    )
                })
                .collect();
            sections.push(KeyedView::new("albums", section("Albums", rows)));
        }

        if !all.artists.results.is_empty() {
            // Artist rows are info-only: top songs need a seed song id.
            let rows: Vec<KeyedView> = all
                .artists
                .results
                .iter()
                .take(5)
                .map(|hit: &SearchArtistHit| {
                    KeyedView::new(
                        hit.id.clone(),
                        StackPanel::new().margin(4.0).children((
                            TextBlock::new()
                                .text(hit.title.clone())
                                .font_size(14.0)
                                .font_weight(FontWeight::SEMI_BOLD),
                            TextBlock::new()
                                .text(hit.description.clone().unwrap_or_default())
                                .font_size(12.0)
                                .opacity(0.7),
                        )),
                    )
                })
                .collect();
            sections.push(KeyedView::new("artists", section("Artists", rows)));
        }

        if !all.playlists.results.is_empty() {
            let rows: Vec<KeyedView> = all
                .playlists
                .results
                .iter()
                .take(5)
                .map(|hit: &SearchPlaylistHit| {
                    KeyedView::new(
                        hit.id.clone(),
                        hit_row(
                            &hit.id,
                            "playlist",
                            &hit.title,
                            hit.description.as_deref().unwrap_or(""),
                            &hit.image,
                            context,
                        ),
                    )
                })
                .collect();
            sections.push(KeyedView::new("playlists", section("Playlists", rows)));
        }

        if sections.is_empty() {
            TextBlock::new()
                .text("No results — try a different search.")
                .font_size(14.0)
                .opacity(0.7)
                .into()
        } else {
            StackPanel::new()
                .spacing(24.0)
                .keyed_children(sections)
                .into()
        }
    } else {
        TextBlock::new()
            .text("Type in the search box above to find songs, albums and artists.")
            .font_size(14.0)
            .opacity(0.7)
            .into()
    };

    ScrollViewer::new()
        .content(
            StackPanel::new().margin(24.0).spacing(16.0).children((
                TextBlock::new()
                    .text("Search")
                    .font_size(28.0)
                    .font_weight(FontWeight::BOLD),
                search_box,
                content,
            )),
        )
        .into()
}
