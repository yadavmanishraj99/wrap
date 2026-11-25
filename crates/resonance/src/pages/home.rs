//! Home page: launch-data sections (Trending now, Top playlists,
//! New albums, Charts) as rows of clickable media cards, via the
//! shared `views::card_section` layout Search and Library use too.
//!
//! Row rule: a section shows at most two rows of cards — one row
//! when it has 10 or fewer items, two rows when it has more (up to
//! 20 items shown). Rows scroll horizontally; clicking a song card
//! plays it, any other card opens its details page.

use saavnapi::LaunchItem;
use windows_reactor::*;

use crate::app::{DetailsTarget, Message, Shell};
use crate::views;

/// Map launch items to card targets.
fn targets(items: &[LaunchItem]) -> Vec<DetailsTarget> {
    items
        .iter()
        .map(|item| DetailsTarget {
            kind: item.kind.clone(),
            id: item.id.clone(),
            title: item.title.clone(),
            subtitle: item.subtitle.clone().unwrap_or_else(|| item.kind.clone()),
            art: views::art_url(&item.image, "500x500"),
        })
        .collect()
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    // A details drill-in is an inner route of this page: when one
    // is open it renders here, inside the page, instead of the
    // page's own content.
    if !shell.details.is_empty() {
        return super::details::view(shell, context);
    }
    let content: View = if shell.home_loading {
        // Skeleton: this page's own layout — the four real sections
        // (Trending now, Top playlists, New albums, Charts) with
        // their headings and a row of skeleton cards each — so the
        // placeholder reads as Home, not a generic stack of bars.
        StackPanel::new().margin(24.0).spacing(32.0).children((
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            views::skeleton_section(shell, "Trending now", 5),
            views::skeleton_section(shell, "Top playlists", 5),
            views::skeleton_section(shell, "New albums", 5),
            views::skeleton_section(shell, "Charts", 5),
        ))
    } else if let Some(err) = &shell.home_error {
        StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text(format!("Couldn't load Home: {err}"))
                .font_size(14.0)
                .opacity(0.7),
            Button::new()
                .on_click(context.message(Message::RetryHome))
                .content("Retry"),
        ))
    } else if let Some(home) = &shell.home {
        // Loaded content fades in (opacity transition on the Border,
        // driven by shell.home_fade: 0 on arrival, 1 a beat later).
        Border::new()
            .opacity(shell.home_fade)
            .opacity_transition(std::time::Duration::from_millis(300))
            .content(
                StackPanel::new().margin(24.0).spacing(32.0).children((
                    TextBlock::new()
                        .text("Home")
                        .font_size(28.0)
                        .font_weight(FontWeight::BOLD),
                    views::card_section(
                        "Trending now",
                        &targets(&home.new_trending),
                        shell,
                        context,
                    ),
                    views::card_section(
                        "Top playlists",
                        &targets(&home.top_playlists),
                        shell,
                        context,
                    ),
                    views::card_section("New albums", &targets(&home.new_albums), shell, context),
                    views::card_section("Charts", &targets(&home.charts), shell, context),
                )),
            )
    } else {
        TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into()
    };

    ScrollViewer::new().content(content)
}
