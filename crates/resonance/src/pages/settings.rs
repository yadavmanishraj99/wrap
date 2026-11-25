//! Settings page: theme, stream quality, autoplay, and about. Every change saves immediately to
//! `%LOCALAPPDATA%\Resonance\settings.json`.

use windows_reactor::*;

use crate::app::{Message, Shell};
use crate::store::{Material, QualitySetting, Theme};
use crate::views;

fn radio(
    label: &str,
    group: &str,
    checked: bool,
    message: Message,
    context: &mut ViewContext<Shell>,
) -> View {
    RadioButton::new()
        .group_name(group)
        .is_checked(checked)
        .on_checked(context.callback(
            move |on: bool| {
                if on { message.clone() } else { Message::Noop }
            },
        ))
        .content(label)
}

/// A labeled toggle switch row (label + ToggleSwitch).
fn toggle_row(
    label: &str,
    is_on: bool,
    message: fn(bool) -> Message,
    context: &mut ViewContext<Shell>,
) -> View {
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(12.0)
        .children((
            TextBlock::new()
                .text(label)
                .font_size(14.0)
                .vertical_alignment(VerticalAlignment::Center),
            ToggleSwitch::new()
                .is_on(is_on)
                .on_toggled(context.callback(move |on: bool| message(on))),
        ))
}

pub fn view(shell: &Shell, context: &mut ViewContext<Shell>) -> View {
    let settings = &shell.settings;

    ScrollViewer::new().content(
        StackPanel::new().margin(24.0).spacing(24.0).children((
            TextBlock::new()
                .text("Settings")
                .font_size(28.0)
                .font_weight(FontWeight::BOLD),
            StackPanel::new().spacing(8.0).children((
                views::section_header("Appearance"),
                radio(
                    "Dark",
                    "theme",
                    settings.theme == Theme::Dark,
                    Message::SetTheme(Theme::Dark),
                    context,
                ),
                radio(
                    "Light",
                    "theme",
                    settings.theme == Theme::Light,
                    Message::SetTheme(Theme::Light),
                    context,
                ),
                radio(
                    "System",
                    "theme",
                    settings.theme == Theme::System,
                    Message::SetTheme(Theme::System),
                    context,
                ),
            )),
            StackPanel::new().spacing(8.0).children((
                views::section_header("Window material"),
                TextBlock::new()
                    .text("Backdrop behind the window. Mica is the default.")
                    .font_size(12.0)
                    .opacity(0.7),
                radio(
                    Material::Mica.label(),
                    "material",
                    settings.material == Material::Mica,
                    Message::SetMaterial(Material::Mica),
                    context,
                ),
                radio(
                    Material::MicaAlt.label(),
                    "material",
                    settings.material == Material::MicaAlt,
                    Message::SetMaterial(Material::MicaAlt),
                    context,
                ),
                radio(
                    Material::Acrylic.label(),
                    "material",
                    settings.material == Material::Acrylic,
                    Message::SetMaterial(Material::Acrylic),
                    context,
                ),
                radio(
                    Material::None.label(),
                    "material",
                    settings.material == Material::None,
                    Message::SetMaterial(Material::None),
                    context,
                ),
            )),
            StackPanel::new().spacing(8.0).children((
                views::section_header("Stream quality"),
                TextBlock::new()
                    .text("Applies to the next song you play.")
                    .font_size(12.0)
                    .opacity(0.7),
                radio(
                    QualitySetting::High.label(),
                    "quality",
                    settings.quality == QualitySetting::High,
                    Message::SetQuality(QualitySetting::High),
                    context,
                ),
                radio(
                    QualitySetting::Medium.label(),
                    "quality",
                    settings.quality == QualitySetting::Medium,
                    Message::SetQuality(QualitySetting::Medium),
                    context,
                ),
                radio(
                    QualitySetting::Low.label(),
                    "quality",
                    settings.quality == QualitySetting::Low,
                    Message::SetQuality(QualitySetting::Low),
                    context,
                ),
            )),
            StackPanel::new().spacing(8.0).children((
                views::section_header("Playback"),
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(12.0)
                    .children((
                        TextBlock::new()
                            .text("Autoplay next track in the queue")
                            .font_size(14.0)
                            .vertical_alignment(VerticalAlignment::Center),
                        ToggleSwitch::new()
                            .is_on(settings.autoplay)
                            .on_toggled(context.callback(|on: bool| Message::SetAutoplay(on))),
                    )),
            )),
            StackPanel::new().spacing(8.0).children((
                views::section_header("Downloads"),
                TextBlock::new()
                    .text("Downloaded songs play from disk, so they work offline.")
                    .font_size(12.0)
                    .opacity(0.7),
                toggle_row(
                    "Download liked songs, albums and playlists",
                    settings.auto_download_liked,
                    Message::SetAutoDownloadLiked,
                    context,
                ),
                toggle_row(
                    "Download pinned items",
                    settings.auto_download_pinned,
                    Message::SetAutoDownloadPinned,
                    context,
                ),
                toggle_row(
                    "Download recently played songs",
                    settings.auto_download_recent,
                    Message::SetAutoDownloadRecent,
                    context,
                ),
                StackPanel::new().spacing(8.0).children((
                    TextBlock::new()
                        .text("Recently played songs kept")
                        .font_size(14.0),
                    radio(
                        "50 songs",
                        "recent-max",
                        settings.recent_max == 50,
                        Message::SetRecentMax(50),
                        context,
                    ),
                    radio(
                        "100 songs",
                        "recent-max",
                        settings.recent_max == 100,
                        Message::SetRecentMax(100),
                        context,
                    ),
                    radio(
                        "200 songs",
                        "recent-max",
                        settings.recent_max == 200,
                        Message::SetRecentMax(200),
                        context,
                    ),
                    radio(
                        "500 songs",
                        "recent-max",
                        settings.recent_max == 500,
                        Message::SetRecentMax(500),
                        context,
                    ),
                )),
                StackPanel::new().spacing(8.0).children((
                    TextBlock::new()
                        .text("Download quality (separate from stream quality)")
                        .font_size(14.0),
                    radio(
                        QualitySetting::High.label(),
                        "download-quality",
                        settings.download_quality == QualitySetting::High,
                        Message::SetDownloadQuality(QualitySetting::High),
                        context,
                    ),
                    radio(
                        QualitySetting::Medium.label(),
                        "download-quality",
                        settings.download_quality == QualitySetting::Medium,
                        Message::SetDownloadQuality(QualitySetting::Medium),
                        context,
                    ),
                    radio(
                        QualitySetting::Low.label(),
                        "download-quality",
                        settings.download_quality == QualitySetting::Low,
                        Message::SetDownloadQuality(QualitySetting::Low),
                        context,
                    ),
                )),
                StackPanel::new().spacing(8.0).children((
                    TextBlock::new()
                        .text(
                            "Image cache size (album art, so images are not downloaded every time)",
                        )
                        .font_size(14.0),
                    radio(
                        "100 MB",
                        "image-cache",
                        settings.image_cache_limit_mb == 100,
                        Message::SetImageCacheLimitMb(100),
                        context,
                    ),
                    radio(
                        "250 MB",
                        "image-cache",
                        settings.image_cache_limit_mb == 250,
                        Message::SetImageCacheLimitMb(250),
                        context,
                    ),
                    radio(
                        "500 MB",
                        "image-cache",
                        settings.image_cache_limit_mb == 500,
                        Message::SetImageCacheLimitMb(500),
                        context,
                    ),
                    radio(
                        "1 GB",
                        "image-cache",
                        settings.image_cache_limit_mb == 1024,
                        Message::SetImageCacheLimitMb(1024),
                        context,
                    ),
                )),
            )),
            StackPanel::new().spacing(8.0).children((
                views::section_header("About"),
                TextBlock::new()
                    .text("Resonance 0.1.0 — music from JioSaavn via the saavnapi crate.")
                    .font_size(14.0)
                    .opacity(0.7),
                TextBlock::new()
                    .text(format!(
                        "Library and settings are stored under {}",
                        crate::store::app_dir().display()
                    ))
                    .font_size(12.0)
                    .opacity(0.7),
            )),
        )),
    )
}
