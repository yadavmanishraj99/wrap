//! Playlists — `playlist.getDetails`, `webapi.get`.
//!
//! Reference: `src/modules/playlists/` (2 use-cases).
//!
//! Note the reference quirk: the by-link use-case calls
//! `Endpoints.albums.link`, not `Endpoints.playlists.link` — both are
//! the string `webapi.get`, so the wire call is identical.

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};
use crate::models::Playlist;
use crate::payload::playlist_payload;

impl SaavnClient {
    /// Get a playlist by ID — `__call=playlist.getDetails`,
    /// `listid`, `n=limit`, `p=page`.
    ///
    /// `listid` is **numeric** (e.g. `82914609`); a token such as
    /// `AMoxtXyKHoU_` is rejected by Saavn with
    /// `INPUT_INVALID: Only numbers are allowed` — use
    /// [`SaavnClient::get_playlist_by_link`] for tokens.
    ///
    /// Like the reference, `song_count` is overwritten with the number
    /// of songs actually returned, and `songs` is sliced to `limit`.
    pub async fn get_playlist_by_id(&self, id: &str, limit: u32, page: u32) -> Result<Playlist> {
        let data = self
            .fetch(
                "playlist.getDetails",
                &[
                    ("listid", id.to_string()),
                    ("n", limit.to_string()),
                    ("p", page.to_string()),
                ],
                Context::Web,
            )
            .await?;
        finish_playlist(data, limit)
    }

    /// Get a playlist by link — `__call=webapi.get`, `token`, `n`, `p`,
    /// `type=playlist`.
    pub async fn get_playlist_by_link(
        &self,
        link: &str,
        limit: u32,
        page: u32,
    ) -> Result<Playlist> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[
                    ("token", token),
                    ("n", limit.to_string()),
                    ("p", page.to_string()),
                    ("type", "playlist".to_string()),
                ],
                Context::Web,
            )
            .await?;
        finish_playlist(data, limit)
    }
}

fn finish_playlist(data: serde_json::Value, limit: u32) -> Result<Playlist> {
    if data.is_null() {
        return Err(Error::NotFound("playlist not found".into()));
    }
    let mut playlist = playlist_payload(&data);
    if let Some(songs) = playlist.songs.take() {
        let mut songs = songs;
        songs.truncate(limit as usize);
        playlist.song_count = Some(songs.len() as u64);
        playlist.songs = Some(songs);
    } else {
        playlist.song_count = Some(0);
    }
    Ok(playlist)
}
