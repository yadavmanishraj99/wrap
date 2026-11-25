//! Search page — filled in by push 3 (typed search, suggestions,
//! song/album/artist/playlist results). Push 1 ships the placeholder.

use windows_reactor::*;

use crate::app::Shell;

pub fn view(_shell: &Shell, _context: &mut ViewContext<Shell>) -> View {
    StackPanel::new()
        .margin(24.0)
        .spacing(12.0)
        .children((
            TextBlock::new()
                .text("Search")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text("Type in the search box above to find songs, albums and artists.")
                .font_size(14.0)
                .opacity(0.7),
        ))
        .into()
}
