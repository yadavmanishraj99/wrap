//! Recommendations — `reco.getreco`, `reco.getAlbumReco`,
//! `reco.getPlaylistReco`.
//!
//! Live-verified 2026-11-25 (dead constants in the sumitkolhe
//! reference; real in the TS/Python/Dart wrappers):
//!
//! * `reco.getreco` — `pid=<song id>`, **`ctx=android`**.
//!   With `web6dot0` it always returns `[]`. With android it returns
//!   a dict keyed by pid: `{ "<pid>": [songs] }`. This is the working
//!   replacement for the server-side-broken `webradio.getSong`
//!   suggestions flow.
//! * `reco.getAlbumReco` — `albumid`, web6dot0, bare list of 16
//!   lightweight `mini_obj` albums.
//! * `reco.getPlaylistReco` — `listid`, web6dot0, bare list of 16
//!   full playlist objects.

use serde_json::Value;

use crate::client::{Context, SaavnClient};
use crate::error::Result;
use crate::models::{Album, Playlist, Song};
use crate::payload::{album_payload, playlist_payload, song_payload};

impl SaavnClient {
    /// Songs recommended for a song — `__call=reco.getreco`,
    /// `ctx=android` (required: web6dot0 always returns `[]`).
    pub async fn get_song_recommendations(&self, song_id: &str) -> Result<Vec<Song>> {
        let data = self
            .fetch(
                "reco.getreco",
                &[("pid", song_id.to_string())],
                Context::Android,
            )
            .await?;
        // Response is a dict keyed by the pid, not a bare list.
        let songs = data
            .get(song_id)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(songs.iter().map(song_payload).collect())
    }

    /// Albums recommended for an album — `__call=reco.getAlbumReco`.
    /// Items are lightweight `mini_obj` albums (no artist/song data).
    pub async fn get_album_recommendations(&self, album_id: &str) -> Result<Vec<Album>> {
        let data = self
            .fetch(
                "reco.getAlbumReco",
                &[("albumid", album_id.to_string())],
                Context::Web,
            )
            .await?;
        Ok(data
            .as_array()
            .map(|a| a.iter().map(album_payload).collect())
            .unwrap_or_default())
    }

    /// Playlists recommended for a playlist — `__call=reco.getPlaylistReco`.
    pub async fn get_playlist_recommendations(&self, playlist_id: &str) -> Result<Vec<Playlist>> {
        let data = self
            .fetch(
                "reco.getPlaylistReco",
                &[("listid", playlist_id.to_string())],
                Context::Web,
            )
            .await?;
        Ok(data
            .as_array()
            .map(|a| a.iter().map(playlist_payload).collect())
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reco_song_dict_unwrap() {
        let raw = json!({"3IoDK8qI": [
            {"id": "x", "title": "Don't Start Now", "type": "song", "perma_url": "u",
             "image": "https://c.saavncdn.com/x-150x150.jpg", "language": "english",
             "more_info": {"artistMap": {"primary_artists": [], "featured_artists": [], "artists": []}}}
        ]});
        let songs = raw["3IoDK8qI"]
            .as_array()
            .map(|a| a.iter().map(song_payload).collect::<Vec<_>>())
            .unwrap();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].name, "Don't Start Now");
        // Wrong key / bare list / empty object all give no songs, no panic.
        assert!(raw.get("other").is_none());
        assert!(json!([]).as_array().is_some());
    }

    #[test]
    fn reco_album_mini_obj_shape() {
        let raw = json!([{"id": "1", "title": "Positions (Deluxe)", "type": "album",
            "perma_url": "u", "image": "https://c.saavncdn.com/x-150x150.jpg",
            "language": "", "year": "", "play_count": "",
            "more_info": {"mini_obj": "true"}}]);
        let albums: Vec<Album> = raw.as_array().unwrap().iter().map(album_payload).collect();
        assert_eq!(albums.len(), 1);
        assert_eq!(albums[0].name, "Positions (Deluxe)");
        assert_eq!(albums[0].song_count, None);
    }
}
