//! Settings page — filled in by push 5 (theme, stream quality,
//! volume, autoplay). Push 1 ships the placeholder.

use windows_reactor::*;

use crate::app::Shell;

pub fn view(_shell: &Shell, _context: &mut ViewContext<Shell>) -> View {
    StackPanel::new()
        .margin(24.0)
        .spacing(12.0)
        .children((
            TextBlock::new()
                .text("Settings")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text("Theme, stream quality and playback options land here next.")
                .font_size(14.0)
                .opacity(0.7),
        ))
        .into()
}
