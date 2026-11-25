//! Shows / podcasts / episodes.
//!
//! Live-verified 2026-11-25. Podcasts resolve as `type=show` — there
//! is no `type=podcast` (it returns `[]`), and `type=channel` /
//! `type=radio_station` are not valid `webapi.get` types either.
//!
//! * `webapi.get`, `type=show` → `{show_details, seasons, episodes,
//!   modules}`. Episodes paginate with `n`/`p` (default 10).
//!   Quirk: `show_details.more_info` is a **stringified Python-style
//!   dict** (single quotes), not a JSON object — kept verbatim.
//! * `webapi.get`, `type=episode` → `{episodes[1], modules}`.
//! * `show.getAllEpisodes` — `show_id`, `season_number`, `p`
//!   (**1-indexed**, `p=0` ≡ `p=1`), `sort_order`.

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};
use crate::models::{Episode, Show};
use crate::payload::{as_array, episode_payload, show_payload};

impl SaavnClient {
    /// A show / podcast by link or bare token —
    /// `__call=webapi.get`, `type=show`.
    pub async fn get_show_by_link(&self, link: &str) -> Result<Show> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[("token", token), ("type", "show".to_string())],
                Context::Web,
            )
            .await?;
        if data.get("show_details").is_none() {
            return Err(Error::NotFound("show not found".into()));
        }
        Ok(show_payload(&data))
    }

    /// One episode by link or bare token —
    /// `__call=webapi.get`, `type=episode`.
    pub async fn get_episode_by_link(&self, link: &str) -> Result<Episode> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[("token", token), ("type", "episode".to_string())],
                Context::Web,
            )
            .await?;
        let episodes = as_array(&data, "episodes");
        episodes
            .first()
            .map(episode_payload)
            .ok_or_else(|| Error::NotFound("episode not found".into()))
    }

    /// All episodes of a show — `__call=show.getAllEpisodes`.
    /// `show_id` is the numeric show id (from `webapi.get` show
    /// details or an episode's `more_info.show_id`). `p` is
    /// 1-indexed; pass 1 for the first page.
    pub async fn get_show_episodes(
        &self,
        show_id: &str,
        season_number: Option<u32>,
        page: u32,
    ) -> Result<Vec<Episode>> {
        let mut params: Vec<(&str, String)> =
            vec![("show_id", show_id.to_string()), ("p", page.to_string())];
        if let Some(season) = season_number {
            params.push(("season_number", season.to_string()));
        }
        let data = self
            .fetch("show.getAllEpisodes", &params, Context::Web)
            .await?;
        Ok(data
            .as_array()
            .map(|a| a.iter().map(episode_payload).collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn show_payload_maps_live_shape() {
        let raw = json!({
            "show_details": {"id": "63565", "title": "Shri Krishna Amritvani", "type": "show",
                "perma_url": "u", "image": "https://c.sop.saavncdn.com/x-500x500.jpg",
                "language": "hindi", "year": "2021", "play_count": "100",
                "explicit_content": "0",
                "more_info": "{'editorial_language': 'hindi'}"},
            "seasons": [{"id": "75395", "title": "Season 1"}],
            "episodes": [{"id": "e1", "title": "Ep 1", "type": "episode",
                "perma_url": "u", "image": "https://c.sop.saavncdn.com/x-500x500.jpg",
                "more_info": {"duration": "600", "season_no": "1", "episode_number": "1",
                    "show_id": "63565", "artistMap": {"primary_artists": [], "featured_artists": [], "artists": []}}}],
            "modules": {"seasons": {}, "episodes": {}}
        });
        let show = show_payload(&raw);
        assert_eq!(show.title, "Shri Krishna Amritvani");
        assert_eq!(show.seasons.len(), 1);
        assert_eq!(show.seasons[0].title, "Season 1");
        assert_eq!(show.episodes.len(), 1);
        assert_eq!(show.episodes[0].duration, Some(600));
        // Python-dict-string more_info is kept verbatim, not parsed.
        assert!(show
            .more_info
            .as_str()
            .unwrap()
            .contains("editorial_language"));
    }

    #[test]
    fn show_payload_missing_details_is_empty() {
        let show = show_payload(&json!({}));
        assert!(show.id.is_empty());
        assert!(show.episodes.is_empty());
    }
}
