//! Library page: liked songs and recently played, persisted locally
//! in `%LOCALAPPDATA%\Resonance\library.json`. Liked albums and
//! playlists render here too (liking them from Search/Home lands in
//! a later push — the store already holds them).

use windows_reactor::*;

use crate::app::Shell;
use crate::views;

/// A song section: heading + keyed song rows, or an empty note.
fn song_section(
    key: &str,
    title: &str,
    songs: &[saavnapi::Song],
    empty_note: &str,
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> KeyedView {
    let body: View = if songs.is_empty() {
        TextBlock::new()
            .text(empty_note)
            .font_size(14.0)
            .opacity(0.7)
            .into()
    } else {
        let rows: Vec<KeyedView> = songs
            .iter()
            .map(|song| {
                KeyedView::new(
                    song.id.clone(),
                    views::song_row(song, songs, shell, context),
                )
            })
            .collect();
        StackPanel::new().spacing(4.0).keyed_children(rows)
    };
    KeyedView::new(
        key,
        StackPanel::new().spacing(4.0).keyed_children(vec![
            KeyedView::new(format!("{key}-header"), views::section_header(title)),
            KeyedView::new(format!("{key}-body"), body),
        ]),
    )
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let liked_albums: View = if shell.library.liked_albums.is_empty() {
        TextBlock::new()
            .text("No liked albums yet.")
            .font_size(14.0)
            .opacity(0.7)
            .into()
    } else {
        let rows: Vec<KeyedView> = shell
            .library
            .liked_albums
            .iter()
            .map(|album| {
                KeyedView::new(
                    album.id.clone(),
                    TextBlock::new().text(album.name.clone()).font_size(14.0),
                )
            })
            .collect();
        StackPanel::new().spacing(4.0).keyed_children(rows)
    };

    let liked_playlists: View = if shell.library.liked_playlists.is_empty() {
        TextBlock::new()
            .text("No liked playlists yet.")
            .font_size(14.0)
            .opacity(0.7)
            .into()
    } else {
        let rows: Vec<KeyedView> = shell
            .library
            .liked_playlists
            .iter()
            .map(|playlist| {
                KeyedView::new(
                    playlist.id.clone(),
                    TextBlock::new().text(playlist.name.clone()).font_size(14.0),
                )
            })
            .collect();
        StackPanel::new().spacing(4.0).keyed_children(rows)
    };

    ScrollViewer::new().content(
        StackPanel::new().margin(24.0).spacing(24.0).children((
            TextBlock::new()
                .text("Your Library")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            StackPanel::new().spacing(24.0).keyed_children(vec![
                song_section(
                    "liked",
                    "Liked songs",
                    &shell.library.liked_songs,
                    "No liked songs yet — tap the heart on any song.",
                    shell,
                    context,
                ),
                song_section(
                    "recent",
                    "Recently played",
                    &shell.library.recent_songs,
                    "Nothing played yet.",
                    shell,
                    context,
                ),
                KeyedView::new(
                    "albums",
                    StackPanel::new().spacing(4.0).keyed_children(vec![
                        KeyedView::new("albums-header", views::section_header("Liked albums")),
                        KeyedView::new("albums-body", liked_albums),
                    ]),
                ),
                KeyedView::new(
                    "playlists",
                    StackPanel::new().spacing(4.0).keyed_children(vec![
                        KeyedView::new(
                            "playlists-header",
                            views::section_header("Liked playlists"),
                        ),
                        KeyedView::new("playlists-body", liked_playlists),
                    ]),
                ),
            ]),
        )),
    )
}
