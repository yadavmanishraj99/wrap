//! Radio station creation — featured and artist stations.
//!
//! Live-verified 2026-11-25 (TS wrapper config + site probe).
//! Entity stations already live in `songs.rs`
//! (`create_song_station`); these two create stations by name.
//! All three return `{stationid}` on success.

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};

fn station_id(data: &serde_json::Value) -> Result<String> {
    match data.get("stationid").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => Ok(id.to_string()),
        _ => Err(Error::InvalidResponse(
            "station creation returned no stationid".into(),
        )),
    }
}

impl SaavnClient {
    /// Create a featured station — `__call=webradio.createFeaturedStation`.
    /// `name` is required; `language` is optional.
    pub async fn create_featured_station(
        &self,
        name: &str,
        language: Option<&str>,
    ) -> Result<String> {
        if name.trim().is_empty() {
            return Err(Error::InvalidInput("station name is required".into()));
        }
        let mut params: Vec<(&str, String)> = vec![("name", name.to_string())];
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("webradio.createFeaturedStation", &params, Context::Web)
            .await?;
        station_id(&data)
    }

    /// Create an artist station — `__call=webradio.createArtistStation`.
    /// Both `name` and `query` are required (query defaults to `name`
    /// in the TS wrapper when empty — pass it explicitly here).
    pub async fn create_artist_station(
        &self,
        name: &str,
        query: &str,
        language: Option<&str>,
    ) -> Result<String> {
        if name.trim().is_empty() || query.trim().is_empty() {
            return Err(Error::InvalidInput(
                "station name and query are required".into(),
            ));
        }
        let mut params: Vec<(&str, String)> =
            vec![("name", name.to_string()), ("query", query.to_string())];
        if let Some(lang) = language {
            params.push(("language", lang.to_string()));
        }
        let data = self
            .fetch("webradio.createArtistStation", &params, Context::Web)
            .await?;
        station_id(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn station_id_extraction() {
        assert_eq!(
            station_id(&json!({"stationid": "abc123"})).unwrap(),
            "abc123"
        );
        assert!(station_id(&json!({})).is_err());
        assert!(station_id(&json!({"stationid": ""})).is_err());
        assert!(station_id(&json!({"error": "x"})).is_err());
    }
}
