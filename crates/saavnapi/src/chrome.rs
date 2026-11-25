//! Web chrome — footer details and the mega menu.
//!
//! Live-verified 2026-11-25.
//! * `webapi.getFooterDetails` — `language` effectively required
//!   (falls back to hindi); returns `{playlist[], artist[], album[],
//!   actor[]}` of `{id, title, action}`.
//! * `webapi.getBrowseHoverDetails` — `language`/`isEntityPage`
//!   are ignored live (byte-identical responses); returns
//!   `{mega_menu: {top_artists[10], top_playlists[10],
//!   new_releases[10]}}`, title + URL only. Note: the
//!   `content.getBrowseHoverDetails` variant does **not** exist
//!   (`INPUT_INVALID`).

use crate::client::{Context, SaavnClient};
use crate::error::Result;
use crate::models::{FooterDetails, MegaMenu};
use crate::payload::{footer_payload, mega_menu_payload};

impl SaavnClient {
    /// Footer links — `__call=webapi.getFooterDetails`.
    pub async fn get_footer_details(&self, language: &str) -> Result<FooterDetails> {
        let data = self
            .fetch(
                "webapi.getFooterDetails",
                &[("language", language.to_string())],
                Context::Web,
            )
            .await?;
        Ok(footer_payload(&data))
    }

    /// Mega menu — `__call=webapi.getBrowseHoverDetails`.
    pub async fn get_browse_hover_details(&self) -> Result<MegaMenu> {
        let data = self
            .fetch("webapi.getBrowseHoverDetails", &[], Context::Web)
            .await?;
        Ok(mega_menu_payload(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn footer_payload_maps_live_shape() {
        let raw = json!({"playlist": [{"id": "1", "title": "Top Hindi", "action": "p"}],
            "artist": [], "album": [{"id": "2", "title": "A", "action": "a"}], "actor": []});
        let footer = footer_payload(&raw);
        assert_eq!(footer.playlists.len(), 1);
        assert_eq!(footer.albums[0].title, "A");
        assert!(footer.artists.is_empty());
    }

    #[test]
    fn mega_menu_payload_accepts_wrapper_and_bare() {
        let wrapped = json!({"mega_menu": {"top_artists": [{"name": "Emraan Hashmi", "url": "u"}],
            "top_playlists": [{"name": "Surprise Me", "url": "u"}], "new_releases": []}});
        let menu = mega_menu_payload(&wrapped);
        assert_eq!(menu.top_artists[0].name, "Emraan Hashmi");
        assert_eq!(menu.top_playlists.len(), 1);
        let bare = json!({"top_artists": [], "top_playlists": [], "new_releases": []});
        assert!(mega_menu_payload(&bare).top_artists.is_empty());
    }
}
