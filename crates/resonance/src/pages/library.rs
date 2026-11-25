//! Library page: liked songs, recently played, liked albums and
//! liked playlists, persisted locally in
//! `%LOCALAPPDATA%\Resonance\library.json`. Every section is a
//! Home-style row of media cards — song cards play, album and
//! playlist cards open their pages. Albums and playlists land here
//! via the Like button on their pages.

use saavnapi::{Album, Playlist, Song};
use windows_reactor::*;

use crate::app::{DetailsTarget, Shell};
use crate::views;

/// A card section, or its heading plus an empty note when there is
/// nothing to show.
fn card_section_or_empty(
    key: &str,
    title: &str,
    targets: &[DetailsTarget],
    empty_note: &str,
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> KeyedView {
    if targets.is_empty() {
        KeyedView::new(
            key,
            StackPanel::new().spacing(8.0).children((
                views::section_header(title),
                TextBlock::new()
                    .text(empty_note)
                    .font_size(14.0)
                    .opacity(0.7),
            )),
        )
    } else {
        KeyedView::new(key, views::card_section(title, targets, shell, context))
    }
}

fn song_targets(songs: &[Song]) -> Vec<DetailsTarget> {
    songs
        .iter()
        .map(|song| DetailsTarget {
            kind: "song".to_string(),
            id: song.id.clone(),
            title: song.name.clone(),
            subtitle: views::artist_names(song),
            art: views::art_url(&song.image, "500x500"),
        })
        .collect()
}

fn album_targets(albums: &[Album]) -> Vec<DetailsTarget> {
    albums
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
        .collect()
}

fn playlist_targets(playlists: &[Playlist]) -> Vec<DetailsTarget> {
    playlists
        .iter()
        .map(|playlist| DetailsTarget {
            kind: "playlist".to_string(),
            id: playlist.id.clone(),
            title: playlist.name.clone(),
            subtitle: playlist.description.clone().unwrap_or_default(),
            art: views::art_url(&playlist.image, "500x500"),
        })
        .collect()
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    ScrollViewer::new().content(
        StackPanel::new().margin(24.0).spacing(32.0).children((
            TextBlock::new()
                .text("Your Library")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            StackPanel::new().spacing(32.0).keyed_children(vec![
                card_section_or_empty(
                    "liked",
                    "Liked songs",
                    &song_targets(&shell.library.liked_songs),
                    "No liked songs yet — tap the heart on any song.",
                    shell,
                    context,
                ),
                card_section_or_empty(
                    "recent",
                    "Recently played",
                    &song_targets(&shell.library.recent_songs),
                    "Nothing played yet.",
                    shell,
                    context,
                ),
                card_section_or_empty(
                    "albums",
                    "Liked albums",
                    &album_targets(&shell.library.liked_albums),
                    "No liked albums yet — tap Like on any album page.",
                    shell,
                    context,
                ),
                card_section_or_empty(
                    "playlists",
                    "Liked playlists",
                    &playlist_targets(&shell.library.liked_playlists),
                    "No liked playlists yet — tap Like on any playlist page.",
                    shell,
                    context,
                ),
            ]),
        )),
    )
}
