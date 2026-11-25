//! Albums — `content.getAlbumDetails`, `webapi.get`.
//!
//! Reference: `src/modules/albums/` (2 use-cases).

use crate::client::{Context, SaavnClient};
use crate::error::{Error, Result};
use crate::models::Album;
use crate::payload::album_payload;

impl SaavnClient {
    /// Get an album by ID — `__call=content.getAlbumDetails`, `albumid`.
    /// The raw response is a single object (not an array).
    pub async fn get_album_by_id(&self, id: &str) -> Result<Album> {
        let data = self
            .fetch(
                "content.getAlbumDetails",
                &[("albumid", id.to_string())],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("album not found".into()));
        }
        Ok(album_payload(&data))
    }

    /// Get an album by link — `__call=webapi.get`, `token`, `type=album`.
    pub async fn get_album_by_link(&self, link: &str) -> Result<Album> {
        let token = Self::token_from_link(link)?;
        let data = self
            .fetch(
                "webapi.get",
                &[("token", token), ("type", "album".to_string())],
                Context::Web,
            )
            .await?;
        if data.is_null() {
            return Err(Error::NotFound("album not found".into()));
        }
        Ok(album_payload(&data))
    }
}
