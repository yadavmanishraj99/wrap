//! Library page — filled in by push 4 (liked songs, liked albums
//! and playlists, recents, all persisted locally). Push 1 ships the
//! placeholder.

use windows_reactor::*;

use crate::app::Shell;

pub fn view(_shell: &Shell, _context: &mut ViewContext<Shell>) -> View {
    StackPanel::new()
        .margin(24.0)
        .spacing(12.0)
        .children((
            TextBlock::new()
                .text("Your Library")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text("Liked songs, albums and playlists live here.")
                .font_size(14.0)
                .opacity(0.7),
        ))
        .into()
}
