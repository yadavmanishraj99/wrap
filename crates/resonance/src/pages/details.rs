//! Details page: what a media card opens. Loading and error states
//! are shared; the loaded content is rendered by the page for its
//! kind — artist, album, playlist/chart, or song — each with its
//! own layout. The title bar's back button returns to the page we
//! came from.

use windows_reactor::*;

use super::{album, artist, playlist, song};
use crate::app::{Message, Shell};
use crate::views;

/// Details page root.
pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let Some(details) = &shell.details else {
        return TextBlock::new()
            .text("Nothing selected.")
            .margin(24.0)
            .into();
    };
    if details.loading {
        // Skeleton: the details page's shape while it loads — a
        // cover, title and subtitle bars, then song rows, shimmering.
        return ScrollViewer::new().content(StackPanel::new().margin(24.0).spacing(12.0).children(
            (
                views::skeleton_art(200.0, shell),
                views::skeleton_bar(220.0, 28.0, shell),
                views::skeleton_bar(160.0, 14.0, shell),
                views::skeleton_row(shell),
                views::skeleton_row(shell),
                views::skeleton_row(shell),
                views::skeleton_row(shell),
            ),
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
    // Loaded content fades in (opacity transition on the Border,
    // driven by shell.details_fade).
    Border::new()
        .opacity(shell.details_fade)
        .opacity_transition(std::time::Duration::from_millis(300))
        .content(ScrollViewer::new().content(page))
}
