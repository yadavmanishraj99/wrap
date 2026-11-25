//! Home page: trending songs, top playlists, new albums and charts
//! from the launch data loaded when the shell opens. Tapping Play on
//! a row fetches its songs in the background and starts the queue.

use saavnapi::LaunchItem;
use windows_reactor::*;

use crate::app::{Message, Shell};
use crate::views;

/// One launch item row: artwork, title/subtitle, Play button.
fn launch_row(item: &LaunchItem, context: &mut ViewContext<Shell>) -> View {
    let art_view: View = match crate::views::art_url(&item.image, "150x150") {
        Some(url) => views::image_from_url(&url)
            .width(48.0)
            .height(48.0)
            .stretch(Stretch::UniformToFill)
            .grid_column(0)
            .into(),
        None => Border::new()
            .width(48.0)
            .height(48.0)
            .grid_column(0)
            .content(
                SymbolIcon::new()
                    .symbol(Symbol::Audio)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center),
            ),
    };

    Grid::new()
        .columns([
            GridLength::Pixel(48.0),
            GridLength::Star(1.0),
            GridLength::Auto,
        ])
        .margin(4.0)
        .children((
            art_view,
            StackPanel::new()
                .margin(12.0)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1)
                .children((
                    TextBlock::new()
                        .text(item.title.clone())
                        .font_size(14.0)
                        .font_weight(FontWeight::SEMI_BOLD),
                    TextBlock::new()
                        .text(item.subtitle.clone().unwrap_or_else(|| item.kind.clone()))
                        .font_size(12.0)
                        .opacity(0.7),
                )),
            Button::new()
                .margin(4.0)
                .grid_column(2)
                .on_click(context.message(Message::PlayLaunchItem(item.clone())))
                .content(SymbolIcon::new().symbol(Symbol::Play)),
        ))
}

/// A section: heading + up to 6 rows.
fn section(title: &str, items: &[LaunchItem], context: &mut ViewContext<Shell>) -> View {
    let mut rows: Vec<KeyedView> = Vec::new();
    rows.push(KeyedView::new("header", views::section_header(title)));
    for item in items.iter().take(6) {
        rows.push(KeyedView::new(item.id.clone(), launch_row(item, context)));
    }
    StackPanel::new().spacing(4.0).keyed_children(rows)
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
            section("Trending now", &home.new_trending, context),
            section("Top playlists", &home.top_playlists, context),
            section("New albums", &home.new_albums, context),
            section("Charts", &home.charts, context),
        ))
    } else {
        TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into()
    };

    ScrollViewer::new().content(content)
}
