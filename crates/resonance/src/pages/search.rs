//! Search page: the full `search_all` result — top result, songs,
//! albums, artists, playlists — each as a Home-style section of
//! media cards. The search box itself lives in the sidebar (the
//! Search nav row is the input); this page only shows results.
//! Clicking a song card plays it; any other card opens its page.

use saavnapi::{SearchAlbumHit, SearchArtistHit, SearchHit, SearchPlaylistHit};
use windows_reactor::*;

use crate::app::{DetailsTarget, Shell};
use crate::views;

fn song_subtitle(hit: &SearchHit) -> String {
    hit.primary_artists
        .clone()
        .or(hit.singers.clone())
        .or(hit.description.clone())
        .unwrap_or_else(|| hit.kind.clone().unwrap_or_default())
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    // A details drill-in is an inner route of this page: when one
    // is open it renders here, inside the page, instead of the
    // page's own content.
    if shell.details.is_some() {
        return super::details::view(shell, context);
    }

    let content: View = if shell.search_loading {
        StackPanel::new().spacing(12.0).children((
            ProgressRing::new().is_active(true),
            TextBlock::new()
                .text("Searching...")
                .font_size(14.0)
                .opacity(0.7),
        ))
    } else if let Some(err) = &shell.search_error {
        TextBlock::new()
            .text(format!("Search failed: {err}"))
            .font_size(14.0)
            .opacity(0.7)
            .into()
    } else if let Some(all) = &shell.search_all {
        let mut sections: Vec<KeyedView> = Vec::new();

        if let Some(top) = all.top_query.results.first() {
            let target = DetailsTarget {
                kind: top.kind.clone().unwrap_or_else(|| "song".to_string()),
                id: top.id.clone(),
                title: top.title.clone(),
                subtitle: song_subtitle(top),
                art: views::art_url(&top.image, "500x500"),
            };
            sections.push(KeyedView::new(
                "top",
                views::card_section("Top result", &[target], shell, context),
            ));
        }

        if !all.songs.results.is_empty() {
            let targets: Vec<DetailsTarget> = all
                .songs
                .results
                .iter()
                .take(20)
                .map(|hit| DetailsTarget {
                    kind: "song".to_string(),
                    id: hit.id.clone(),
                    title: hit.title.clone(),
                    subtitle: song_subtitle(hit),
                    art: views::art_url(&hit.image, "500x500"),
                })
                .collect();
            sections.push(KeyedView::new(
                "songs",
                views::card_section("Songs", &targets, shell, context),
            ));
        }

        if !all.albums.results.is_empty() {
            let targets: Vec<DetailsTarget> = all
                .albums
                .results
                .iter()
                .take(10)
                .map(|hit: &SearchAlbumHit| DetailsTarget {
                    kind: "album".to_string(),
                    id: hit.id.clone(),
                    title: hit.title.clone(),
                    subtitle: hit.artist.clone().unwrap_or_default(),
                    art: views::art_url(&hit.image, "500x500"),
                })
                .collect();
            sections.push(KeyedView::new(
                "albums",
                views::card_section("Albums", &targets, shell, context),
            ));
        }

        if !all.artists.results.is_empty() {
            let targets: Vec<DetailsTarget> = all
                .artists
                .results
                .iter()
                .take(10)
                .map(|hit: &SearchArtistHit| DetailsTarget {
                    kind: "artist".to_string(),
                    id: hit.id.clone(),
                    title: hit.title.clone(),
                    subtitle: hit.description.clone().unwrap_or_default(),
                    art: views::art_url(&hit.image, "500x500"),
                })
                .collect();
            sections.push(KeyedView::new(
                "artists",
                views::card_section("Artists", &targets, shell, context),
            ));
        }

        if !all.playlists.results.is_empty() {
            let targets: Vec<DetailsTarget> = all
                .playlists
                .results
                .iter()
                .take(10)
                .map(|hit: &SearchPlaylistHit| DetailsTarget {
                    kind: "playlist".to_string(),
                    id: hit.id.clone(),
                    title: hit.title.clone(),
                    subtitle: hit.description.clone().unwrap_or_default(),
                    art: views::art_url(&hit.image, "500x500"),
                })
                .collect();
            sections.push(KeyedView::new(
                "playlists",
                views::card_section("Playlists", &targets, shell, context),
            ));
        }

        if sections.is_empty() {
            TextBlock::new()
                .text("No results — try a different search.")
                .font_size(14.0)
                .opacity(0.7)
                .into()
        } else {
            StackPanel::new().spacing(32.0).keyed_children(sections)
        }
    } else {
        TextBlock::new()
            .text("Type in the search box in the sidebar to find songs, albums and artists.")
            .font_size(14.0)
            .opacity(0.7)
            .into()
    };

    ScrollViewer::new().content(
        StackPanel::new().margin(24.0).spacing(16.0).children((
            TextBlock::new()
                .text("Search")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            content,
        )),
    )
}
