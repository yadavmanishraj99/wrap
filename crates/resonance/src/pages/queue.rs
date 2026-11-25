//! Queue page: the current playback queue ("Up next"). Rows reuse
//! `song_row`, so Play starts the queue at that song. Reached from
//! the player bar's queue button; the title-bar back button returns
//! to the page we came from.

use windows_reactor::*;

use crate::app::Shell;
use crate::views;

/// Queue page root.
pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let queue = &shell.player.queue;
    if queue.is_empty() {
        return StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text("Queue")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text("The queue is empty.")
                .font_size(14.0)
                .opacity(0.7),
            TextBlock::new()
                .text("Play a song, album or playlist to fill it.")
                .font_size(14.0)
                .opacity(0.7),
        ));
    }
    let now_playing = shell
        .player
        .current()
        .map(|song| format!("Now playing: {}", song.name))
        .unwrap_or_default();
    let rows: Vec<KeyedView> = queue
        .iter()
        .map(|song| {
            KeyedView::new(
                song.id.clone(),
                views::song_row(song, queue, shell, context),
            )
        })
        .collect();
    ScrollViewer::new().content(
        StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text("Queue")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text(now_playing)
                .font_size(14.0)
                .opacity(0.7)
                .text_wrapping(TextWrapping::Wrap),
            StackPanel::new().spacing(4.0).keyed_children(rows),
        )),
    )
}
