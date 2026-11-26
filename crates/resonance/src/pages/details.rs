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
    // The top of the details stack is what renders; the rest is
    // history that Back walks through in reverse.
    let Some(details) = shell.details.last() else {
        return TextBlock::new()
            .text("Nothing selected.")
            .margin(24.0)
            .into();
    };
    if details.loading {
        // Skeleton: this kind's own layout, not a generic stack of
        // bars — the header (art shape + eyebrow/title/subtitle/
        // button shapes) and the sections that kind carries, so the
        // placeholder reads as the page that is about to appear.
        let kind = details.target.kind.as_str();
        // The header's right column: eyebrow, title, then the
        // lines this kind shows, then its Play/Like buttons.
        let mut header_lines: Vec<KeyedView> = vec![
            KeyedView::new("eyebrow", views::skeleton_bar(60.0, 12.0, shell)),
            KeyedView::new("title", views::skeleton_bar(220.0, 28.0, shell)),
        ];
        // Artist has one Play button; every other kind has Play +
        // Like side by side.
        let buttons: View = if kind == "artist" {
            views::skeleton_button(110.0, 36.0, shell)
        } else {
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .children((
                    views::skeleton_button(110.0, 36.0, shell),
                    views::skeleton_button(90.0, 36.0, shell),
                ))
        };
        match kind {
            "artist" => {
                header_lines.push(KeyedView::new(
                    "subtitle",
                    views::skeleton_bar(160.0, 14.0, shell),
                ));
            }
            "album" => {
                header_lines.push(KeyedView::new(
                    "artists",
                    views::skeleton_bar(180.0, 14.0, shell),
                ));
                header_lines.push(KeyedView::new(
                    "meta",
                    views::skeleton_bar(120.0, 14.0, shell),
                ));
            }
            "playlist" | "chart" => {
                header_lines.push(KeyedView::new(
                    "desc",
                    views::skeleton_bar(200.0, 14.0, shell),
                ));
                header_lines.push(KeyedView::new(
                    "count",
                    views::skeleton_bar(80.0, 14.0, shell),
                ));
            }
            _ => {
                // Song (and anything else): title + artists.
                header_lines.push(KeyedView::new(
                    "artists",
                    views::skeleton_bar(180.0, 14.0, shell),
                ));
            }
        }
        header_lines.push(KeyedView::new("buttons", buttons));
        // The sections below the header, per kind.
        let mut sections: Vec<KeyedView> = vec![KeyedView::new(
            "header",
            header_skeleton(shell, kind == "artist", header_lines),
        )];
        match kind {
            "artist" => {
                sections.push(KeyedView::new(
                    "songs",
                    StackPanel::new().spacing(4.0).children((
                        views::section_header("Top songs"),
                        StackPanel::new().spacing(4.0).children((
                            views::skeleton_row(shell),
                            views::skeleton_row(shell),
                            views::skeleton_row(shell),
                            views::skeleton_row(shell),
                        )),
                    )),
                ));
                sections.push(KeyedView::new(
                    "albums",
                    views::skeleton_section(shell, "Top albums", 5),
                ));
                sections.push(KeyedView::new(
                    "similar",
                    views::skeleton_section(shell, "Similar artists", 5),
                ));
            }
            "album" | "playlist" | "chart" => {
                sections.push(KeyedView::new(
                    "songs",
                    StackPanel::new().spacing(4.0).children((
                        views::section_header("Songs"),
                        StackPanel::new().spacing(4.0).children((
                            views::skeleton_row(shell),
                            views::skeleton_row(shell),
                            views::skeleton_row(shell),
                            views::skeleton_row(shell),
                            views::skeleton_row(shell),
                        )),
                    )),
                ));
            }
            _ => {
                // Song: the lyrics block (a heading + text lines).
                sections.push(KeyedView::new(
                    "lyrics",
                    StackPanel::new().spacing(8.0).children((
                        views::section_header("Lyrics"),
                        views::skeleton_bar(320.0, 12.0, shell),
                        views::skeleton_bar(280.0, 12.0, shell),
                        views::skeleton_bar(300.0, 12.0, shell),
                        views::skeleton_bar(240.0, 12.0, shell),
                    )),
                ));
            }
        }
        return ScrollViewer::new().content(
            StackPanel::new()
                .margin(24.0)
                .spacing(24.0)
                .keyed_children(sections),
        );
    }
    if details.error.is_some() {
        return StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text(details.target.title.clone())
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text("Couldn't load this page. Check your connection and try again.")
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

/// The header of a details page as a skeleton: the art (circular
/// for an artist, square otherwise) in a 200px grid cell, and the
/// right column (the eyebrow / title / subtitle / button shapes),
/// the same shape the real header has.
fn header_skeleton(shell: &Shell, circular: bool, lines: Vec<KeyedView>) -> View {
    let art: View = Border::new()
        .width(200.0)
        .height(200.0)
        .corner_radius(CornerRadius::uniform(if circular { 100.0 } else { 4.0 }))
        .grid_column(0)
        .background(views::skeleton_fill(shell.settings.theme))
        .into();
    Grid::new()
        .columns([GridLength::Pixel(200.0), GridLength::Star(1.0)])
        .children((
            art,
            StackPanel::new()
                .margin(24.0)
                .spacing(8.0)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(1)
                .keyed_children(lines),
        ))
}
