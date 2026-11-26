//! Home page: every launch-data section except History, as
//! rows of clickable media cards, via the shared
//! `views::card_section` layout Search and Library use too.
//! Section order and titles come from the launch data's own
//! `modules` config (Saavn's presentation order); sections it
//! doesn't cover (Browse discover, stray promo sections) are
//! appended after.
//!
//! Row rule: a section shows at most two rows of cards — one row
//! when it has 10 or fewer items, two rows when it has more (up
//! to 20 items shown). Rows scroll horizontally; clicking a song
//! card plays it, any other card opens its details page — except
//! the kinds the app can't open yet (channels, radio stations,
//! shows), which explain that in a plain-language notice.

use saavnapi::LaunchItem;
use windows_reactor::*;

use crate::app::{DetailsTarget, Message, Shell};
use crate::views;

/// Map launch items to card targets.
fn targets(items: &[LaunchItem]) -> Vec<DetailsTarget> {
    targets_as(items, None)
}

/// Map launch items to card targets, overriding the kind —
/// artist-station items carry the artist's own id, so their
/// cards open the artist page as kind "artist".
fn targets_as(items: &[LaunchItem], kind: Option<&str>) -> Vec<DetailsTarget> {
    items
        .iter()
        .map(|item| DetailsTarget {
            kind: kind.unwrap_or(&item.kind).to_string(),
            id: item.id.clone(),
            title: item.title.clone(),
            subtitle: item.subtitle.clone().unwrap_or_else(|| item.kind.clone()),
            art: views::art_url(&item.image, "500x500"),
        })
        .collect()
}

/// Section title when the modules config doesn't name one.
fn fallback_title(source: &str) -> String {
    match source {
        "new_trending" => "Trending Now",
        "charts" => "Top Charts",
        "new_albums" => "New Releases",
        "top_playlists" => "Editorial Picks",
        "radio" => "Radio Stations",
        "artist_recos" => "Recommended Artist Stations",
        "browse_discover" => "Discover",
        _ => "Featured",
    }
    .to_string()
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    // A details drill-in is an inner route of this page: when one
    // is open it renders here, inside the page, instead of the
    // page's own content.
    if !shell.details.is_empty() {
        return super::details::view(shell, context);
    }
    let content: View = if shell.home_loading {
        // Skeleton: this page's own layout — the first real
        // sections with their headings and a row of skeleton
        // cards each — so the placeholder reads as Home, not a
        // generic stack of bars.
        StackPanel::new().margin(24.0).spacing(32.0).children((
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            views::skeleton_section(shell, "Trending Now", 5),
            views::skeleton_section(shell, "Top Charts", 5),
            views::skeleton_section(shell, "New Releases", 5),
            views::skeleton_section(shell, "Editorial Picks", 5),
        ))
    } else if shell.home_error.is_some() {
        StackPanel::new().margin(24.0).spacing(12.0).children((
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            TextBlock::new()
                .text("Couldn't load Home. Check your connection and try again.")
                .font_size(14.0)
                .opacity(0.7),
            Button::new()
                .on_click(context.message(Message::RetryHome))
                .content("Retry"),
        ))
    } else if let Some(home) = &shell.home {
        // (position, key, title, targets) per section, in the
        // modules config's order. History is the one section
        // left out.
        let mut sections: Vec<(i64, String, String, Vec<DetailsTarget>)> = Vec::new();
        let mut covered: Vec<String> = Vec::new();
        for (key, cfg) in &home.modules {
            let source = cfg.source.as_deref().unwrap_or(key.as_str());
            if source == "history" {
                continue;
            }
            let (items, as_kind): (&[LaunchItem], Option<&str>) = match source {
                "new_trending" => (&home.new_trending, None),
                "top_playlists" => (&home.top_playlists, None),
                "new_albums" => (&home.new_albums, None),
                "charts" => (&home.charts, None),
                "browse_discover" => (&home.browse_discover, None),
                "radio" => (&home.radio, None),
                "artist_recos" => (&home.artist_recos, Some("artist")),
                _ => match home.promo_sections.get(source) {
                    Some(items) => (items.as_slice(), None),
                    None => continue,
                },
            };
            if items.is_empty() {
                continue;
            }
            covered.push(source.to_string());
            let title = cfg
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| fallback_title(source));
            sections.push((
                cfg.position,
                source.to_string(),
                title,
                targets_as(items, as_kind),
            ));
        }
        // Sections the modules config doesn't mention: any stray
        // promo sections, then Browse discover, go last.
        for (key, items) in &home.promo_sections {
            if !covered.contains(key) && !items.is_empty() {
                sections.push((
                    i64::MAX - 1,
                    key.clone(),
                    fallback_title(key),
                    targets(items),
                ));
            }
        }
        if !covered.contains(&"browse_discover".to_string()) && !home.browse_discover.is_empty() {
            sections.push((
                i64::MAX,
                "browse_discover".to_string(),
                fallback_title("browse_discover"),
                targets(&home.browse_discover),
            ));
        }
        sections.sort_by_key(|(position, _, _, _)| *position);

        let mut children: Vec<KeyedView> = vec![KeyedView::new(
            "home-title",
            TextBlock::new()
                .text("Home")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
        )];
        for (_, key, title, section_targets) in &sections {
            children.push(KeyedView::new(
                format!("section-{key}"),
                views::card_section(title, section_targets, shell, context),
            ));
        }
        StackPanel::new()
            .margin(24.0)
            .spacing(32.0)
            .keyed_children(children)
    } else {
        TextBlock::new()
            .text("Nothing here yet.")
            .margin(24.0)
            .into()
    };

    ScrollViewer::new().content(content)
}
