//! Songs — `song.getDetails`, `webapi.get`, `webradio.*`.
//!
//! Reference: `src/modules/songs/` (service + 4 use-cases).

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};
use crate::models::Song;
use crate::payload::{as_array, song_payload};

impl SaavnClient {
    /// Get songs by ID — `__call=song.getDetails`, `pids=<comma-joined>`.
    /// Returns an array even for a single ID, like the reference.
    /// 404 when `songs` is empty/missing.
    pub async fn get_songs_by_ids(&self, ids: &[&str]) -> Result<Vec<Song>> {
        if ids.is_empty() {
            return Err(Error::InvalidInput(
                "at least one song id is required".into(),
            ));
        }
        let data = self
            .fetch("song.getDetails", &[("pids", ids.join(","))], Context::Web)
            .await?;
        let songs = as_array(&data, "songs");
        if songs.is_empty() {
            return Err(Error::NotFound("song not found".into()));
        }
        Ok(songs.iter().map(song_payload).collect())
    }

    /// Get one song by link — `__call=webapi.get`, `token`, `type=song`.
    /// Accepts a full `https://www.jiosaavn.com/song/…/<token>` URL or a
    /// bare token. Returns an array, like the reference.
    pub async fn get_songs_by_link(&self, link: &str) -> Result<Vec<Song>> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[("token", token), ("type", "song".to_string())],
                Context::Web,
            )
            .await?;
        let songs = as_array(&data, "songs");
        if songs.is_empty() {
            return Err(Error::NotFound("song not found".into()));
        }
        Ok(songs.iter().map(song_payload).collect())
    }

    /// Create a radio station for a song — `__call=webradio.createEntityStation`,
    /// `ctx=android`, `entity_id=["<url-encoded id>"]`, `entity_type=queue`.
    ///
    /// Internal in the reference (no public route); exposed here because
    /// `get_song_suggestions` is a two-step flow and callers may want the
    /// station id itself.
    pub async fn create_song_station(&self, song_id: &str) -> Result<String> {
        // Reference: JSON.stringify([encodeURIComponent(songId)])
        let encoded = url_encode(song_id);
        let entity_id = format!("[\"{encoded}\"]");
        let data = self
            .fetch(
                "webradio.createEntityStation",
                &[
                    ("entity_id", entity_id),
                    ("entity_type", "queue".to_string()),
                ],
                Context::Android,
            )
            .await?;
        match data.get("stationid").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => Ok(id.to_string()),
            _ => Err(Error::Station),
        }
    }

    /// Song suggestions — two steps, exactly like the reference:
    /// 1. `create_song_station(song_id)` → `stationid`
    /// 2. `__call=webradio.getSong`, `ctx=android`, `stationid`, `k=limit`
    ///
    /// The raw response is `{ stationid, <key>: { song } }` — the
    /// `stationid` key is stripped, each `{ song }` is mapped, and the
    /// result is sliced to `limit` (default 10 in the controller).
    pub async fn get_song_suggestions(&self, song_id: &str, limit: u32) -> Result<Vec<Song>> {
        let station_id = self.create_song_station(song_id).await?;
        let data = self
            .fetch(
                "webradio.getSong",
                &[("stationid", station_id), ("k", limit.to_string())],
                Context::Android,
            )
            .await?;
        let Some(obj) = data.as_object() else {
            return Err(Error::NotFound(
                "no suggestions found for the given song".into(),
            ));
        };
        // Live behaviour (2026-11-25, verified 4× across two songs):
        // Saavn currently answers `{ stationid, error: "No new song
        // found for current radio." }` here. Surface that instead of
        // a generic not-found, so callers can tell a dead endpoint
        // from a bad song id. The reference's own suggestions test is
        // commented out, consistent with this being server-side.
        if let Some(err) = obj.get("error").and_then(|v| v.as_str()) {
            return Err(Error::InvalidResponse(format!("webradio.getSong: {err}")));
        }
        let mut songs: Vec<Song> = obj
            .iter()
            .filter(|(k, _)| *k != "stationid")
            .filter_map(|(_, v)| v.get("song"))
            .map(song_payload)
            .collect();
        songs.truncate(limit as usize);
        if songs.is_empty() {
            return Err(Error::NotFound(
                "no suggestions found for the given song".into(),
            ));
        }
        Ok(songs)
    }
}

/// Minimal percent-encoding for the station `entity_id` — the reference
/// uses `encodeURIComponent`, which leaves `A-Z a-z 0-9 - _ . ! ~ * ' ( )`
/// unescaped. Song ids are alphanumeric, so this is effectively a no-op
/// for real ids, but it is correct for the general case.
fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
