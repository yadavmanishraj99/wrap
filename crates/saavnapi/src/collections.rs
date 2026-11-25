//! Mixes and labels — `webapi.get` with `type=mix` / `type=label`.
//!
//! From the TS wrapper config (live shape not yet probed with a real
//! token — mix/label links are rare in search results; see the unit
//! tests for the assumed shape, taken from the wrapper's payloads).
//! A mix is a user/curated song list; a label page lists a label's
//! top songs and top albums.

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};
use crate::models::{Label, Mix};
use crate::payload::{label_payload, mix_payload};

impl SaavnClient {
    /// A mix by link or bare token — `__call=webapi.get`, `type=mix`.
    pub async fn get_mix_by_link(&self, link: &str) -> Result<Mix> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[("token", token), ("type", "mix".to_string())],
                Context::Web,
            )
            .await?;
        if data.get("id").is_none() {
            return Err(Error::NotFound("mix not found".into()));
        }
        Ok(mix_payload(&data))
    }

    /// A label by link or bare token — `__call=webapi.get`, `type=label`.
    pub async fn get_label_by_link(&self, link: &str) -> Result<Label> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[("token", token), ("type", "label".to_string())],
                Context::Web,
            )
            .await?;
        if data.get("id").is_none() && data.get("labelId").is_none() {
            return Err(Error::NotFound("label not found".into()));
        }
        Ok(label_payload(&data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mix_payload_maps_wrapper_shape() {
        let raw = json!({"id": "m1", "title": "Chill Mix", "type": "mix",
            "perma_url": "u", "image": "https://c.saavncdn.com/x-150x150.jpg",
            "list_count": "3", "list": [
                {"id": "s1", "title": "Song One", "type": "song", "perma_url": "u",
                 "image": "https://c.saavncdn.com/x-150x150.jpg", "language": "english",
                 "more_info": {"artistMap": {"primary_artists": [], "featured_artists": [], "artists": []}}}],
            "more_info": {"username": "jio", "firstname": "JioSaavn"}});
        let mix = mix_payload(&raw);
        assert_eq!(mix.title, "Chill Mix");
        assert_eq!(mix.songs.len(), 1);
        assert_eq!(mix.songs[0].name, "Song One");
        assert_eq!(mix.username.as_deref(), Some("jio"));
    }

    #[test]
    fn label_payload_maps_wrapper_shape() {
        let raw = json!({"labelId": "1606707", "name": "Warner Records", "type": "label",
            "perma_url": "u", "image": "https://c.saavncdn.com/x-150x150.jpg",
            "top_songs": {"songs": [
                {"id": "s1", "title": "Song One", "type": "song", "perma_url": "u",
                 "image": "https://c.saavncdn.com/x-150x150.jpg", "language": "english",
                 "more_info": {"artistMap": {"primary_artists": [], "featured_artists": [], "artists": []}}}],
                "total": 42},
            "top_albums": {"albums": [], "total": 0},
            "availableLanguages": ["english", "hindi"]});
        let label = label_payload(&raw);
        assert_eq!(label.id, "1606707");
        assert_eq!(label.top_songs.len(), 1);
        assert_eq!(label.top_songs_total, Some(42));
        assert_eq!(label.available_languages, vec!["english", "hindi"]);
    }
}
