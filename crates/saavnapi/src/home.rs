//! Home / launch data — `webapi.getLaunchData`, `content.getBrowseModules`,
//! `content.getTrending`.
//!
//! "Launch data" is the JioSaavn home page payload: what the app shows
//! on launch — trending now, new releases, top playlists, charts,
//! radio, artist recommendations, promos. It was **not** in the
//! `sumitkolhe/jiosaavn-api` reference at all (only the dead constants
//! `content.getBrowseModules` / `content.getTrending`, with no use-case
//! or route). The endpoint itself is real and documented by other
//! clients (e.g. `rajput-hemant/jiosaavn-api-ts`:
//! `modules.launch_data = "webapi.getLaunchData"`), and verified live
//! here on 2026-11-25.
//!
//! Wire call: `__call=webapi.getLaunchData`, `ctx=web6dot0`, optional
//! `language=<lang>` (e.g. `english` — adds a `city_mod` section).
//! No other parameters are needed.
//!
//! The sections are deliberately heterogeneous — `new_trending` mixes
//! albums, songs and playlists; `browse_discover` is all `channel`s;
//! `radio` / `artist_recos` are `radio_station`s; `promo:*` keys are
//! dynamic (`promo:vx:data:68`, …) — so every section is a
//! [`LaunchItem`] (the common fields) with the raw `more_info`
//! preserved, rather than forcing each section into one entity type.

use std::collections::HashMap;

use serde_json::Value;

use crate::client::{Context, SaavnClient};
use crate::error::Result;
use crate::models::{LaunchData, LaunchItem};
use crate::payload::{launch_items, module_config_payload};

impl SaavnClient {
    /// Home / launch data — `__call=webapi.getLaunchData`.
    ///
    /// `language` is optional (`None` = Saavn's default, which is
    /// what the reference's other calls effectively get). Passing
    /// `Some("english")` adds a `city_mod` section, captured in
    /// `LaunchData.extra`.
    pub async fn get_launch_data(&self, language: Option<&str>) -> Result<LaunchData> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("webapi.getLaunchData", &params, Context::Web)
            .await?;
        Ok(launch_data_payload(&data))
    }

    /// Browse / discover modules — `__call=content.getBrowseModules`.
    ///
    /// Returns the raw section map because the sections are not
    /// uniform: most are lists of items, but `radio` is
    /// `{ featured_stations: […] }` and `top_shows` is
    /// `{ badge, shows, last_page }`. Callers should read the section
    /// they need from the map (values are `serde_json::Value`).
    pub async fn get_browse_modules(&self) -> Result<HashMap<String, Value>> {
        let data = self
            .fetch("content.getBrowseModules", &[], Context::Web)
            .await?;
        Ok(data
            .as_object()
            .cloned()
            .map(|o| o.into_iter().collect())
            .unwrap_or_default())
    }

    /// Trending — `__call=content.getTrending`, `p=page`, `n=limit`.
    /// The `new_trending` module's `view_more` points here
    /// (`default_size: 10`). Items are mixed types (album/song/
    /// playlist), so they are [`LaunchItem`]s, not albums.
    pub async fn get_trending(&self, page: u32, limit: u32) -> Result<Vec<LaunchItem>> {
        let data = self
            .fetch(
                "content.getTrending",
                &[("p", page.to_string()), ("n", limit.to_string())],
                Context::Web,
            )
            .await?;
        Ok(data.as_array().map(|a| launch_items(a)).unwrap_or_default())
    }
}

fn launch_data_payload(v: &Value) -> LaunchData {
    let known = [
        "history",
        "new_trending",
        "top_playlists",
        "new_albums",
        "browse_discover",
        "global_config",
        "charts",
        "radio",
        "artist_recos",
        "city_mod",
        "modules",
    ];
    let section = |key: &str| -> Vec<LaunchItem> {
        v.get(key)
            .and_then(|x| x.as_array())
            .map(|a| launch_items(a))
            .unwrap_or_default()
    };
    let mut promo_sections = HashMap::new();
    let mut extra = HashMap::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if k.starts_with("promo:") {
                if let Some(a) = val.as_array() {
                    promo_sections.insert(k.clone(), launch_items(a));
                }
            } else if !known.contains(&k.as_str()) {
                extra.insert(k.clone(), val.clone());
            }
        }
    }
    let mut modules = HashMap::new();
    if let Some(obj) = v.get("modules").and_then(|x| x.as_object()) {
        for (k, val) in obj {
            modules.insert(k.clone(), module_config_payload(val));
        }
    }
    LaunchData {
        history: section("history"),
        new_trending: section("new_trending"),
        top_playlists: section("top_playlists"),
        new_albums: section("new_albums"),
        browse_discover: section("browse_discover"),
        charts: section("charts"),
        radio: section("radio"),
        artist_recos: section("artist_recos"),
        city_mod: section("city_mod"),
        promo_sections,
        modules,
        global_config: v.get("global_config").cloned().unwrap_or(Value::Null),
        extra,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payload::launch_item_payload;
    use serde_json::json;

    /// Real `webapi.getLaunchData` response, captured 2026-11-25
    /// (language=english). Frozen on disk so the test does not depend
    /// on the live API drifting.
    const ENGLISH_FIXTURE: &str = include_str!("../tests/fixtures/launch_data_english.json");
    const HINDI_FIXTURE: &str = include_str!("../tests/fixtures/launch_data_hindi.json");

    #[test]
    fn parses_real_english_launch_response() {
        let raw: Value = serde_json::from_str(ENGLISH_FIXTURE).unwrap();
        let data = launch_data_payload(&raw);
        // Exact counts for the frozen fixture (live counts drift).
        assert_eq!(data.new_trending.len(), 24);
        assert_eq!(data.new_albums.len(), 20);
        assert_eq!(data.top_playlists.len(), 12);
        assert_eq!(data.charts.len(), 9);
        assert_eq!(data.radio.len(), 28);
        assert_eq!(data.artist_recos.len(), 20);
        assert_eq!(data.browse_discover.len(), 28);
        assert_eq!(data.promo_sections["promo:vx:data:68"].len(), 9);
        assert_eq!(data.promo_sections["promo:vx:data:76"].len(), 7);
        assert_eq!(data.promo_sections["promo:vx:data:185"].len(), 7);
        assert_eq!(data.promo_sections["promo:vx:data:107"].len(), 29);
        assert!(data.city_mod.is_empty()); // english returns no city_mod
        assert_eq!(data.modules.len(), 10);
        assert!(!data.global_config.is_null());

        // Every item in every section carries the basics.
        let all: Vec<&LaunchItem> = data
            .new_trending
            .iter()
            .chain(&data.new_albums)
            .chain(&data.top_playlists)
            .chain(&data.charts)
            .chain(&data.radio)
            .chain(&data.artist_recos)
            .chain(&data.browse_discover)
            .chain(data.promo_sections.values().flatten())
            .collect();
        assert_eq!(all.len(), 193);
        for item in &all {
            assert!(!item.id.is_empty(), "item with empty id: {item:?}");
            assert!(!item.title.is_empty(), "item with empty title: {item:?}");
            assert!(!item.kind.is_empty(), "item with empty type: {item:?}");
            assert!(!item.url.as_deref().unwrap_or("").is_empty());
            // Image present on the wire -> exactly 3 sizes.
            if !item.image.is_empty() {
                assert_eq!(item.image.len(), 3);
                assert!(item.image.iter().all(|l| l.url.starts_with("https://")));
            }
        }

        // Mixed types really are mixed.
        let trending_kinds: std::collections::HashSet<&str> =
            data.new_trending.iter().map(|i| i.kind.as_str()).collect();
        assert!(trending_kinds.contains("album"));
        assert!(trending_kinds.contains("song"));
        assert!(trending_kinds.contains("playlist"));

        // `more_info: []` (promo show items) parses with empty artists.
        let show = &data.promo_sections["promo:vx:data:107"][0];
        assert_eq!(show.kind, "show");
        assert!(show.more_info.is_array());
        assert!(show.artists.all.is_empty());

        // Charts have no `explicit_content` on the wire -> false, and
        // a numeric `count`.
        assert!(data.charts.iter().all(|c| !c.explicit_content));
        assert_eq!(data.charts[0].count, Some(50));

        // Module positions are 1..=10, unique, and view_more is a
        // dict for some modules and `[]` (=> None) for others.
        let mut positions: Vec<i64> = data.modules.values().map(|m| m.position).collect();
        positions.sort_unstable();
        assert_eq!(positions, (1..=10).collect::<Vec<_>>());
        assert_eq!(
            data.modules["new_trending"].view_more.as_ref().unwrap().api,
            "content.getTrending"
        );
        assert_eq!(
            data.modules["new_trending"]
                .view_more
                .as_ref()
                .unwrap()
                .default_size,
            Some(10)
        );
        for key in [
            "promo:vx:data:68",
            "promo:vx:data:76",
            "promo:vx:data:185",
            "artist_recos",
        ] {
            assert!(data.modules[key].view_more.is_none(), "{key} view_more");
        }
    }

    #[test]
    fn parses_real_hindi_launch_response_with_city_mod() {
        let raw: Value = serde_json::from_str(HINDI_FIXTURE).unwrap();
        let data = launch_data_payload(&raw);
        assert_eq!(data.city_mod.len(), 15);
        assert_eq!(data.modules.len(), 11);
        assert!(data.modules.contains_key("city_mod"));
        assert!(data.city_mod.iter().all(|i| !i.title.is_empty()));
        assert!(!data.new_trending.is_empty());
    }

    #[test]
    fn parses_real_browse_modules_fixture() {
        let raw: Value =
            serde_json::from_str(include_str!("../tests/fixtures/browse_modules.json")).unwrap();
        let sections = raw.as_object().unwrap();
        let mut keys: Vec<&str> = sections.keys().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "browse_discover",
                "charts",
                "new_albums",
                "new_trending",
                "radio",
                "top_playlists",
                "top_shows"
            ]
        );
        // Heterogeneous sections: radio is a dict, top_shows is a dict.
        assert!(sections["radio"].get("featured_stations").is_some());
        assert!(sections["top_shows"].get("shows").is_some());
        assert!(sections["new_trending"].as_array().unwrap().len() == 24);
    }

    #[test]
    fn parses_real_trending_fixture() {
        let raw: Value =
            serde_json::from_str(include_str!("../tests/fixtures/trending.json")).unwrap();
        let items: Vec<LaunchItem> = raw
            .as_array()
            .unwrap()
            .iter()
            .map(launch_item_payload)
            .collect();
        assert_eq!(items.len(), 10);
        let kinds: std::collections::HashSet<&str> =
            items.iter().map(|i| i.kind.as_str()).collect();
        assert!(kinds.len() > 1, "trending is mixed types: {kinds:?}");
    }

    #[test]
    fn launch_data_survives_malformed_input() {
        // Anything Saavn (or a proxy) could return must not panic.
        let bad = [
            json!(null),
            json!({}),
            json!([]),
            json!("text"),
            json!(42),
            json!({"new_trending": "not-a-list"}),
            json!({"new_trending": [null, 42, "str", {}, []]}),
            json!({"modules": "junk"}),
            json!({"modules": {"x": "junk", "y": null, "z": []}}),
            json!({"modules": {"x": {"position": "3", "view_more": []}}}),
            json!({"promo:vx:data:1": "not-a-list"}),
            json!({"global_config": null, "extra_key": {"a": 1}}),
        ];
        for v in &bad {
            let data = launch_data_payload(v);
            // No panic; sections default to empty.
            assert!(data.new_trending.is_empty());
            assert!(data.history.is_empty());
        }
        // The string-position case above really does parse.
        let data = launch_data_payload(&json!({"modules": {"x": {"position": "3"}}}));
        assert_eq!(data.modules["x"].position, 3);
    }

    #[test]
    fn launch_item_survives_malformed_input() {
        let bad = [
            json!(null),
            json!([]),
            json!("text"),
            json!(42),
            json!({}),
            json!({"more_info": []}),
            json!({"more_info": "str"}),
            json!({"more_info": 5}),
            json!({"more_info": {"artistMap": "junk"}}),
            json!({"more_info": {"artistMap": {"artists": "junk"}}}),
            json!({"id": 123, "title": true, "type": null, "explicit_content": true}),
            json!({"play_count": 12.5, "count": "50", "image": 99}),
            json!({"count": 50.0, "play_count": "not-a-number"}),
        ];
        for v in &bad {
            let item = launch_item_payload(v);
            // No panic, and the string-count form parses.
            if v.get("count").and_then(|c| c.as_str()) == Some("50") {
                assert_eq!(item.count, Some(50));
            }
            // Junk artistMap never produces artists.
            if v.get("more_info")
                .and_then(|m| m.get("artistMap"))
                .map(|a| a.is_string())
                .unwrap_or(false)
            {
                assert!(item.artists.all.is_empty());
            }
        }
        let item = launch_item_payload(&json!({"id": "1", "title": "t", "type": "song",
            "more_info": {"song_count": "8", "artistMap": {"primary_artists": [], "featured_artists": [], "artists": []}}}));
        assert_eq!(item.song_count, Some(8));
    }

    #[test]
    fn module_config_survives_malformed_input() {
        use crate::payload::module_config_payload;
        let bad = [
            json!(null),
            json!([]),
            json!("text"),
            json!({}),
            json!({"view_more": []}),
            json!({"view_more": "x"}),
            json!({"view_more": {"api": 5}}),
            json!({"position": 3.5}),
            json!({"position": "7", "view_more": {"api": "content.getTrending", "default_size": "10"}}),
        ];
        for v in &bad {
            let _ = module_config_payload(v); // must not panic
        }
        let m = module_config_payload(
            &json!({"position": "7", "view_more": {"api": "content.getTrending", "default_size": "10"}}),
        );
        assert_eq!(m.position, 7);
        assert_eq!(m.view_more.unwrap().default_size, Some(10));
    }

    #[test]
    fn launch_data_serde_roundtrip() {
        let raw: Value = serde_json::from_str(ENGLISH_FIXTURE).unwrap();
        let data = launch_data_payload(&raw);
        let text = serde_json::to_string(&data).unwrap();
        let back: LaunchData = serde_json::from_str(&text).unwrap();
        assert_eq!(data, back);
    }

    #[test]
    fn launch_data_payload_maps_mixed_sections() {
        let raw = json!({
            "history": [],
            "new_trending": [{
                "id": "79121261", "title": "Awarapan 2", "type": "album",
                "perma_url": "https://www.jiosaavn.com/album/awarapan-2/e9NTAB1tQ9M_",
                "image": "https://c.saavncdn.com/820/x-150x150.jpg",
                "language": "hindi", "explicit_content": "0",
                "more_info": {"song_count": "8", "artistMap": {"primary_artists": [], "featured_artists": [], "artists": []}}
            }],
            "top_playlists": [], "new_albums": [], "browse_discover": [],
            "charts": [{"id": "1", "title": "Top 50", "type": "playlist", "count": 50,
                "perma_url": "https://example.com/x", "image": "https://c.saavncdn.com/x-150x150.jpg"}],
            "radio": [], "artist_recos": [],
            "promo:vx:data:68": [{"id": "9", "title": "Promo", "type": "playlist",
                "perma_url": "https://example.com/p", "image": "https://c.saavncdn.com/p-150x150.jpg"}],
            "global_config": {"weekly_top_songs_listid": {}},
            "modules": {"new_trending": {"source": "new_trending", "position": 1, "title": "Trending Now",
                "scroll_type": "SS_Condensed_Double",
                "view_more": {"api": "content.getTrending", "page_param": "p", "size_param": "n", "default_size": 10}}}
        });
        let data = launch_data_payload(&raw);
        assert_eq!(data.new_trending.len(), 1);
        assert_eq!(data.new_trending[0].title, "Awarapan 2");
        assert_eq!(data.new_trending[0].song_count, Some(8));
        assert_eq!(data.charts[0].count, Some(50));
        assert_eq!(data.promo_sections["promo:vx:data:68"].len(), 1);
        assert_eq!(data.modules["new_trending"].position, 1);
        assert_eq!(
            data.modules["new_trending"].view_more.as_ref().unwrap().api,
            "content.getTrending"
        );
    }
}
