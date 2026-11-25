//! Details page: what a media card opens. Loading and error states
//! are shared; the loaded content is rendered by the page for its
//! kind — artist, album, playlist/chart, or song — each with its
//! own layout. The back button in the NavigationView header returns
//! to the page we came from.

use windows_reactor::*;

use super::{album, artist, playlist, song};
use crate::app::{Message, Shell};

/// Details page root.
pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let Some(details) = &shell.details else {
        return TextBlock::new()
            .text("Nothing selected.")
            .margin(24.0)
            .into();
    };
    if details.loading {
        return StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text(details.target.title.clone())
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            ProgressRing::new().is_active(true),
            TextBlock::new()
                .text("Loading details...")
                .font_size(14.0)
                .opacity(0.7),
        ));
    }
    if let Some(err) = &details.error {
        return StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text(details.target.title.clone())
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text(format!("Couldn't load details: {err}"))
                .font_size(14.0)
                .opacity(0.7),
            Button::new()
                .on_click(context.message(Message::Back))
                .content("Back"),
        ));
    }
    let Some(content) = &details.content else {
        return TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into();
    };

    let page = match details.target.kind.as_str() {
        "artist" => artist::view(shell, context, &details.target, content),
        "album" => album::view(shell, context, &details.target, content),
        "playlist" => playlist::view(shell, context, &details.target, content, "Playlist"),
        "chart" => playlist::view(shell, context, &details.target, content, "Chart"),
        "song" => song::view(shell, context, &details.target, content),
        _ => playlist::view(shell, context, &details.target, content, "Playlist"),
    };
    ScrollViewer::new().content(page)
}
