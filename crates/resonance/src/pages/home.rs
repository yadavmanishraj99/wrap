//! Home page: launch-data sections (Trending now, Top playlists,
//! New albums, Charts) as rows of clickable media cards.
//!
//! Row rule: a section shows at most two rows of cards — one row
//! when it has 10 or fewer items, two rows when it has more (up to
//! 20 items shown). Rows scroll horizontally; clicking a card opens
//! its details page.

use saavnapi::LaunchItem;
use windows_reactor::*;

use crate::app::{DetailsTarget, Message, Shell};
use crate::views;

/// One section: heading + up to 20 cards in one row (<= 10 items) or
/// two rows (> 10 items), scrolling horizontally.
fn section(
    title: &str,
    items: &[LaunchItem],
    shell: &Shell,
    context: &mut ViewContext<Shell>,
) -> View {
    let shown: Vec<&LaunchItem> = items.iter().take(20).collect();
    if shown.is_empty() {
        return View::empty();
    }
    let mut row = |slice: &[&LaunchItem]| -> View {
        let cards: Vec<KeyedView> = slice
            .iter()
            .map(|item| {
                let target = DetailsTarget {
                    kind: item.kind.clone(),
                    id: item.id.clone(),
                    title: item.title.clone(),
                    subtitle: item.subtitle.clone().unwrap_or_else(|| item.kind.clone()),
                    art: views::art_url(&item.image, "500x500"),
                };
                KeyedView::new(
                    item.id.clone(),
                    views::media_card(
                        target,
                        views::art_url(&item.image, "500x500"),
                        shell.hovered_card.as_deref() == Some(item.id.as_str()),
                        context,
                    ),
                )
            })
            .collect();
        StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(4.0)
            .keyed_children(cards)
    };
    let body: View = if shown.len() <= 10 {
        row(&shown)
    } else {
        let (first, second) = shown.split_at(10);
        StackPanel::new()
            .spacing(12.0)
            .children((row(first), row(second)))
    };
    StackPanel::new().spacing(8.0).children((
        views::section_header(title),
        ScrollViewer::new()
            .horizontal_scroll_bar_visibility(ScrollBarVisibility::Hidden)
            .vertical_scroll_bar_visibility(ScrollBarVisibility::Disabled)
            .content(body),
    ))
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let content: View = if shell.home_loading {
        StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            ProgressRing::new().is_active(true),
            TextBlock::new()
                .text("Loading trending music...")
                .font_size(14.0)
                .opacity(0.7),
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
        StackPanel::new().margin(24.0).spacing(24.0).children((
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            section("Trending now", &home.new_trending, shell, context),
            section("Top playlists", &home.top_playlists, shell, context),
            section("New albums", &home.new_albums, shell, context),
            section("Charts", &home.charts, shell, context),
        ))
    } else {
        TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into()
    };

    ScrollViewer::new().content(content)
}
