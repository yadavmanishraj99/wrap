//! Home page — filled in by push 2 (launch data, charts, trending).
//! Push 1 ships the placeholder so the shell navigates cleanly.

use windows_reactor::*;

use crate::app::Shell;

pub fn view(_shell: &Shell, _context: &mut ViewContext<Shell>) -> View {
    StackPanel::new()
        .margin(24.0)
        .spacing(12.0)
        .children((
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text("Trending songs, new albums and charts land here next.")
                .font_size(14.0)
                .opacity(0.7),
        ))
        .into()
}
